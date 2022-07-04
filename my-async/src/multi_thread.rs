use super::BoxedFuture;

use super::reactor;
use super::schedulers::{ScheduleMessage, Scheduler, Spawner};

use std::{
    future::Future,
    io,
    sync::Arc,
    thread::{self, JoinHandle},
};

use flume::{Receiver, Sender, TryRecvError};
use once_cell::sync::{Lazy, OnceCell};
use parking_lot::Mutex;
use sharded_slab::Pool;

// Use `OnceCell` to achieve lock-free.
static SPAWNER: OnceCell<Spawner> = OnceCell::new();
// global future allocation pool.
pub static FUTURE_POOL: Lazy<Pool<BoxedFuture>> = Lazy::new(Pool::new);

pub struct Executor<S: Scheduler> {
    scheduler: S,
    poll_thread_notifier: Sender<()>,
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
        let size = if cpus == 0 { 1 } else { cpus };
        let (spawner, scheduler) = S::init(size);
        log::debug!("Scheduler initialized");
        let (tx, rx) = flume::unbounded();
        // set up spawner
        SPAWNER.get_or_init(move || spawner);
        let poll_thread_handle = thread::Builder::new()
            .name("poll_thread".to_string())
            .spawn(move || Self::poll_thread(rx))
            .expect("Failed to spawn poll_thread.");
        log::debug!("Spawned poll_thread");
        log::info!("Runtime startup complete.");
        Self {
            scheduler,
            poll_thread_notifier: tx,
            poll_thread_handle,
        }
    }
    fn poll_thread(rx: Receiver<()>) {
        let mut reactor = reactor::Reactor::default();
        reactor.setup_registry();
        loop {
            match rx.try_recv() {
                // exit signal
                Ok(()) | Err(TryRecvError::Disconnected) => break,
                // channel is empty, exit
                Err(TryRecvError::Empty) => {}
            };
            // check if wakeups that is not used immediately is needed now.
            reactor.check_extra_wakeups();
            if let Err(e) = reactor.wait(None) {
                log::error!("reactor wait error: {}, exit poll thread", e);
                break;
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
                // Err(RecvTimeoutError::Timeout) => continue,
                Err(_) => {
                    log::debug!("exit...");
                    break;
                }
            }
        }
        log::info!("Execution completed, shutting down...");
        // shutdown worker threads
        self.scheduler.shutdown();
        // shutdown poll thread
        self.poll_thread_notifier
            .send(())
            .expect("Failed to send notify");
        self.poll_thread_handle
            .join()
            .expect("Failed to join poll thread");
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
