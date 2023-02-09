//! Single-threaded executor.
//!
//! This executor is capable of executing futures without priority.
//! To spawn a future on it, use [`spawn()`] to add a future to its queue, and use
//! [`Executor::block_on()`] to block on a single main future until it's finished.
//!
//! You need to at least block on one future in order to run this executor.
//!
//! # Example
//!
//! The example shows how you should write your async code using this executor:
//!
//! ```no_run
//! use std::io;
//!
//! use my_async::{
//!     single_thread::{spawn, Executor},
//! };
//!
//! async fn block_future() -> io::Result<()> {
//!     loop {
//!         // ... do stuff
//!         spawn(async move{
//!             handle().await?;
//!             Ok(())
//!         }); // spawn future
//!     }
//!     Ok(())
//! }
//!
//! async fn handle() -> io::Result<()> {
//!     // ... do stuff
//!     Ok(())
//! }
//!
//! fn main() -> io::Result<()> {
//!     let mut rt: Executor = Executor::new(); // create executor instance.
//!     rt.block_on(block_future())
//! }
//! ```

use super::reactor;

use std::{
    cell::RefCell,
    collections::VecDeque,
    future::Future,
    hash::Hash,
    io,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
    time::Duration,
};

use flume::{Receiver, Sender, TryRecvError};
use sharded_slab::{Clear, Pool};
use waker_fn::waker_fn;

thread_local! {
    static SPAWNER: RefCell<Option<Spawner>> = RefCell::new(None);
    static FUTURE_POOL: Pool<BoxedFuture> = Pool::new();
}

type BoxedLocal<T> = Pin<Box<dyn Future<Output = T> + 'static>>;

#[derive(Clone, Copy, Eq)]
struct FutureIndex {
    key: usize,
}

impl PartialEq for FutureIndex {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl Hash for FutureIndex {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.key.hash(state);
    }
}

struct BoxedFuture {
    future: RefCell<Option<BoxedLocal<io::Result<()>>>>,
}

impl Default for BoxedFuture {
    fn default() -> Self {
        BoxedFuture {
            future: RefCell::new(None),
        }
    }
}

impl Clear for BoxedFuture {
    fn clear(&mut self) {
        self.future.borrow_mut().clear();
    }
}

impl BoxedFuture {
    fn run(&self, index: &FutureIndex, tx: Sender<FutureIndex>) -> bool {
        let mut guard = self.future.borrow_mut();
        // run *ONCE*
        if let Some(fut) = guard.as_mut() {
            let new_index = FutureIndex {
                key: index.key,
                // sleep_count: index.sleep_count + 1,
            };
            let waker = waker_fn(move || {
                tx.send(new_index).expect("Too many message queued!");
            });
            let cx = &mut Context::from_waker(&waker);
            match fut.as_mut().poll(cx) {
                Poll::Ready(r) => {
                    if let Err(e) = r {
                        log::error!("Error occurred when executing future: {}", e);
                    }
                    true
                }
                Poll::Pending => false,
            }
        } else {
            true
        }
    }
}

enum Message {
    Run(FutureIndex),
    Close,
}

/// Executor that can run futures.
pub struct Executor {
    task_tx: Sender<FutureIndex>,
    task_rx: Receiver<FutureIndex>,
    queue: VecDeque<FutureIndex>,
    rx: Receiver<Message>,
}

struct Spawner {
    tx: Sender<Message>,
}

impl Executor {
    /// Create executor instance for [`spawn()`] and [`Executor::block_on()`].
    ///
    /// Yout should create the [`Executor`] instance first before trying to spawn any future,
    /// otherwise it won't have any effect.
    pub fn new() -> Self {
        let (tx, rx) = flume::unbounded();
        let (task_tx, task_rx) = flume::unbounded();
        let spawner = Spawner { tx };
        SPAWNER.with(|s| s.borrow_mut().replace(spawner));
        Self {
            task_tx,
            task_rx,
            queue: VecDeque::with_capacity(1024),
            rx,
        }
    }

    fn run(&mut self) {
        let mut reactor = reactor::Reactor::default();
        reactor.setup_registry();
        'outer: loop {
            if let Some(index) = self.queue.pop_back() {
                FUTURE_POOL.with(|p| {
                    if let Some(boxed) = p.get(index.key) {
                        let finished = boxed.run(&index, self.task_tx.clone());
                        if finished && !p.clear(index.key) {
                            log::error!(
                                "Failed to remove completed future with index = {} from pool.",
                                index.key
                            );
                        }
                    } else {
                        log::error!("Future with index = {} is not in pool.", index.key);
                    }
                });
            } else {
                let mut wakeup_count = 0;
                loop {
                    match self.task_rx.try_recv() {
                        Ok(index) => {
                            wakeup_count += 1;
                            self.queue.push_front(index);
                        }
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => break 'outer,
                    }
                }
                if wakeup_count > 0 {
                    continue;
                }
                match self.rx.try_recv() {
                    Ok(Message::Run(index)) => {
                        self.queue.push_front(index);
                    }
                    Err(TryRecvError::Empty) => {
                        if let Err(e) = reactor.wait(Some(Duration::from_millis(50)), || false) {
                            log::error!("reactor wait error: {}, exit", e);
                            break;
                        }
                    }
                    Ok(Message::Close) | Err(TryRecvError::Disconnected) => break,
                }
            }
        }
    }
    /// Blocks on a single future.
    ///
    /// The executor will continue to run until this future finishes.
    ///
    /// You can imagine that this will execute the "main" future function
    /// to complete before the executor shutdown.
    pub fn block_on<F>(mut self, future: F) -> F::Output
    where
        F: Future + 'static,
    {
        let result_arc: Rc<RefCell<Option<F::Output>>> = Rc::new(RefCell::new(None));
        let clone = Rc::clone(&result_arc);
        spawn(async move {
            let result = future.await;
            // should put any result inside the arc, even if it's `()`!
            clone.borrow_mut().replace(result);
            log::debug!("Blocked future finished.");
            shutdown();
            Ok(())
        });
        log::info!("Start blocking...");
        self.run();
        log::debug!("Waiting result...");
        let mut guard = result_arc.borrow_mut();
        let result = guard.take();
        assert!(
            result.is_some(),
            "The blocked future should produce a return value before the execution ends."
        );
        result.unwrap()
    }
}

impl Drop for Executor {
    fn drop(&mut self) {
        SPAWNER.with(|s| {
            if let Some(spawner) = s.borrow().as_ref() {
                spawner
                    .tx
                    .send(Message::Close)
                    .expect("Message queue is full.");
            }
        });
    }
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}

impl Spawner {
    fn spawn<F>(&self, future: F)
    where
        F: Future<Output = io::Result<()>> + 'static,
    {
        let key = FUTURE_POOL.with(|p| {
            p.create_with(|seat| {
                seat.future.borrow_mut().replace(Box::pin(future));
            })
            .unwrap()
        });
        self.tx
            .send(Message::Run(FutureIndex { key }))
            .expect("too many task queued");
    }
    fn shutdown(&self) {
        self.tx.send(Message::Close).expect("too many task queued");
    }
}

/// Spawns a future to executor's queue.
///
/// Note that until you create [`Executor`] instance,
/// this fuction won't have any effect.
///
/// Currently doesn't implement join handle in single thread.
pub fn spawn<F>(fut: F)
where
    F: Future<Output = io::Result<()>> + 'static,
{
    SPAWNER.with(|s| {
        if let Some(spawner) = s.borrow().as_ref() {
            spawner.spawn(fut);
        }
    })
}

/// Notify the executor to shutdown.
///
/// Useful for some early exit condition. E.g. error handling.
pub fn shutdown() {
    SPAWNER.with(|s| {
        if let Some(spawner) = s.borrow().as_ref() {
            spawner.shutdown();
        }
    })
}
