use super::reactor;

use std::{
    collections::VecDeque,
    future::Future,
    hash::Hash,
    io,
    sync::Arc,
    task::{Context, Poll},
};

use flume::{Receiver, Sender, TryRecvError};
use futures_lite::future::Boxed;
use futures_lite::future::FutureExt;
use once_cell::sync::{Lazy, OnceCell};
use parking_lot::Mutex;
use sharded_slab::{Clear, Pool};
use waker_fn::waker_fn;

static SPAWNER: OnceCell<Spawner> = OnceCell::new();
static FUTURE_POOL: Lazy<Pool<BoxedFuture>> = Lazy::new(Pool::new);

#[derive(Clone, Copy, Eq)]
pub struct FutureIndex {
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

#[allow(dead_code)]
pub struct BoxedFuture {
    future: Mutex<Option<Boxed<io::Result<()>>>>,
    sleep_count: usize,
}

impl Default for BoxedFuture {
    fn default() -> Self {
        BoxedFuture {
            future: Mutex::new(None),
            sleep_count: 0,
        }
    }
}

impl Clear for BoxedFuture {
    fn clear(&mut self) {
        self.future.get_mut().clear();
    }
}

impl BoxedFuture {
    pub fn run(&self, index: &FutureIndex, tx: Sender<FutureIndex>) -> bool {
        let mut guard = self.future.lock();
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
    pub fn new() -> Self {
        let (tx, rx) = flume::unbounded();
        let (task_tx, task_rx) = flume::unbounded();
        let spawner = Spawner { tx };
        SPAWNER.get_or_init(move || spawner);
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
                if let Some(boxed) = FUTURE_POOL.get(index.key) {
                    let finished = boxed.run(&index, self.task_tx.clone());
                    if finished && !FUTURE_POOL.clear(index.key) {
                        log::error!(
                            "Failed to remove completed future with index = {} from pool.",
                            index.key
                        );
                    }
                } else {
                    log::error!("Future with index = {} is not in pool.", index.key);
                }
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
                        if let Err(e) = reactor.wait(None) {
                            log::error!("reactor wait error: {}, exit", e);
                            break;
                        }
                    }
                    Ok(Message::Close) | Err(TryRecvError::Disconnected) => break,
                }
            }
        }
    }
    pub fn block_on<F, T>(mut self, future: F) -> T
    where
        T: Send + 'static,
        F: Future<Output = T> + Send + 'static,
    {
        let result_arc: Arc<Mutex<Option<T>>> = Arc::new(Mutex::new(None));
        let clone = Arc::clone(&result_arc);
        spawn(async move {
            let result = future.await;
            // should put any result inside the arc, even if it's `()`!
            clone.lock().replace(result);
            log::debug!("Blocked future finished.");
            shutdown();
            Ok(())
        });
        log::info!("Start blocking...");
        self.run();
        log::debug!("Waiting result...");
        let mut guard = result_arc.lock();
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
        if let Some(spawner) = SPAWNER.get() {
            spawner
                .tx
                .send(Message::Close)
                .expect("Message queue is full.");
        }
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
        F: Future<Output = io::Result<()>> + 'static + Send,
    {
        let key = FUTURE_POOL
            .create_with(|seat| {
                seat.future.get_mut().replace(future.boxed());
            })
            .unwrap();
        self.tx
            .send(Message::Run(FutureIndex {
                key,
                // sleep_count: 0,
            }))
            .expect("too many task queued");
    }
    fn shutdown(&self) {
        self.tx.send(Message::Close).expect("too many task queued");
    }
}

pub fn spawn<F>(fut: F)
where
    F: Future<Output = io::Result<()>> + 'static + Send,
{
    if let Some(spawner) = SPAWNER.get() {
        spawner.spawn(fut);
    }
}

pub fn shutdown() {
    if let Some(spawner) = SPAWNER.get() {
        spawner.shutdown();
    }
}
