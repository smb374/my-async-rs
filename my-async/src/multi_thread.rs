use super::reactor::{self, POLL_WAKER};
use super::schedulers::{ScheduleMessage, Scheduler, Spawner};

use std::{
    future::Future,
    hash::Hash,
    io, panic,
    sync::Arc,
    task::{Context, Poll},
    thread::{self, JoinHandle},
    time::Duration,
};

use flume::Sender;
use futures_lite::future::Boxed;
use once_cell::sync::{Lazy, OnceCell};
use parking_lot::Mutex;
use sharded_slab::{Clear, Pool};
use waker_fn::waker_fn;

pub type WrappedTaskSender = Option<Sender<FutureIndex>>;

#[derive(Clone, Copy, Eq)]
pub struct FutureIndex {
    pub(crate) key: usize,
    pub(crate) sleep_count: usize,
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
    pub(crate) future: Mutex<Option<Boxed<io::Result<()>>>>,
    pub(crate) sleep_count: usize,
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
                sleep_count: index.sleep_count + 1,
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

// Use `OnceCell` to achieve lock-free.
static SPAWNER: OnceCell<Spawner> = OnceCell::new();
// global future allocation pool.
pub static FUTURE_POOL: Lazy<Pool<BoxedFuture>> = Lazy::new(Pool::new);

pub struct Executor<S: Scheduler> {
    scheduler: S,
    poll_thread_handle: JoinHandle<()>,
}

impl<S: Scheduler> Default for Executor<S> {
    fn default() -> Executor<S> {
        Executor::new()
    }
}

impl<S: Scheduler> Executor<S> {
    pub fn new() -> Self {
        let cpus = num_cpus::get();
        assert!(
            cpus > 0,
            "Failed to detect core number of current cpu, panic!"
        );
        let size = cpus;
        let (spawner, scheduler) = S::init(size);
        log::debug!("Scheduler initialized");
        // set up spawner
        SPAWNER.get_or_init(move || spawner);
        let poll_thread_handle = thread::Builder::new()
            .name("poll_thread".to_string())
            .spawn(move || Self::poll_thread())
            .expect("Failed to spawn poll_thread.");
        log::debug!("Spawned poll_thread");
        panic::set_hook(Box::new(|info| {
            let message = match info.payload().downcast_ref::<&str>() {
                Some(s) => s,
                None => "No panic message",
            };
            let location = match info.location() {
                Some(l) => format!("at {}, line {}", l.file(), l.line()),
                None => "No location information".to_string(),
            };
            eprintln!(
                "Runtime panic: message: {}, location: {}",
                message, location
            );
            log::error!("Runtime panic, request shutting down...");
            shutdown();
        }));
        log::info!("Runtime startup complete.");
        Self {
            scheduler,
            poll_thread_handle,
        }
    }
    fn poll_thread() {
        let mut reactor = reactor::Reactor::default();
        reactor.setup_registry();
        loop {
            // check if wakeups that is not used immediately is needed now.
            reactor.check_extra_wakeups();
            match reactor.wait(Some(Duration::from_millis(100))) {
                Ok(true) => break,
                Ok(false) => continue,
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => {
                    log::error!("reactor wait error: {}, exit poll thread", e);
                    break;
                }
            }
        }
    }
    fn run(mut self) {
        log::info!("Runtime booted up, start execution...");
        loop {
            match self.scheduler.receiver().recv() {
                // continously schedule tasks
                Ok(msg) => match msg {
                    ScheduleMessage::Schedule(future) => self.scheduler.schedule(future),
                    ScheduleMessage::Reschedule(task) => self.scheduler.reschedule(task),
                    ScheduleMessage::Shutdown => break,
                },
                Err(_) => {
                    log::debug!("exit...");
                    break;
                }
            }
        }
        log::info!("Execution completed, shutting down...");
        // shutdown worker threads
        self.scheduler.shutdown();
        match POLL_WAKER.get() {
            Some(waker) => {
                if let Err(e) = waker.wake() {
                    log::error!("Failed to wake poll_thread: {}", e);
                } else {
                    self.poll_thread_handle
                        .join()
                        .expect("Failed to join poll thread");
                    log::debug!("poll thread shutdown completed.");
                }
            }
            None => log::error!("POLL_WAKER not set when trying to join poll thread!"),
        }
        log::info!("Runtime shutdown complete.")
    }
    pub fn block_on<F, T>(self, future: F) -> T
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

pub fn spawn<F>(future: F)
where
    F: Future<Output = io::Result<()>> + Send + 'static,
{
    if let Some(spawner) = SPAWNER.get() {
        spawner.spawn(future);
    }
}

pub fn shutdown() {
    if let Some(spawner) = SPAWNER.get() {
        spawner.shutdown();
    }
}
