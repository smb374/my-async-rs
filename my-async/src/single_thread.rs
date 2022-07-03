use super::{reactor, BoxedFuture, FutureIndex};

use std::{collections::VecDeque, future::Future, io, sync::Arc};

use flume::{Receiver, Sender, TryRecvError};
use futures_lite::future::FutureExt;
use once_cell::sync::{Lazy, OnceCell};
use parking_lot::Mutex;
use sharded_slab::Pool;

static SPAWNER: OnceCell<Spawner> = OnceCell::new();
static FUTURE_POOL: Lazy<Pool<BoxedFuture>> = Lazy::new(Pool::new);

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
                if let Some(boxed) = FUTURE_POOL.get(index) {
                    let finished = boxed.run(index, self.task_tx.clone());
                    if finished && !FUTURE_POOL.clear(index) {
                        tracing::error!(
                            "Failed to remove completed future with index = {} from pool.",
                            index
                        );
                    }
                } else {
                    tracing::error!("Future with index = {} is not in pool.", index);
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
                            tracing::error!("reactor wait error: {}, exit", e);
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
            tracing::debug!("Blocked future finished.");
            shutdown();
            Ok(())
        });
        tracing::info!("Start blocking...");
        self.run();
        tracing::debug!("Waiting result...");
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

impl Spawner {
    fn spawn<F>(&self, future: F)
    where
        F: Future<Output = io::Result<()>> + 'static + Send,
    {
        let index = FUTURE_POOL
            .create_with(|seat| {
                seat.0.get_mut().replace(future.boxed());
            })
            .unwrap();
        self.tx
            .send(Message::Run(index))
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
