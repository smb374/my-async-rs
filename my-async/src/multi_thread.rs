use crate::schedulers::BoxedFuture;

use super::reactor;
use super::schedulers::{ScheduleMessage, Scheduler, Spawner};

use std::{
    io,
    ops::Deref,
    sync::Arc,
    thread::{self, JoinHandle},
};

use flume::{Receiver, Sender, TryRecvError};
use futures_lite::prelude::*;
use once_cell::sync::Lazy;
use parking_lot::{Mutex, RwLock};
use sharded_slab::Pool;
use tracing::metadata::LevelFilter;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

// write only when initializing, using `RwLock` for frequent multiple read access.
// NOTE: using `Once` and unsafe to initialize the spawner may be a faster choice since it only mutate once & without needing any locks.
static SPAWNER: Lazy<RwLock<Option<Spawner>>> = Lazy::new(|| RwLock::new(None));
// global future allocation pool.
pub static FUTURE_POOL: Lazy<Pool<BoxedFuture>> = Lazy::new(|| Pool::new());

pub struct Executor<S: Scheduler> {
    scheduler: S,
    poll_thread_notifier: Sender<()>,
    poll_thread_handle: JoinHandle<()>,
}

impl<S: Scheduler> Executor<S> {
    pub fn new() -> Self {
        // log format
        let format = fmt::layer()
            .with_level(true)
            .with_target(false)
            .with_thread_ids(false)
            .with_thread_names(true);
        // log level filter, default showing warning & above.
        let filter = EnvFilter::builder()
            .with_default_directive(LevelFilter::WARN.into())
            .from_env_lossy();
        // init tracing subscriber as loggging facility.
        tracing_subscriber::registry()
            .with(format)
            .with(filter)
            .init();
        let cpus = num_cpus::get();
        let size = if cpus == 0 { 1 } else { cpus };
        let (spawner, scheduler) = S::init(size);
        tracing::debug!("Scheduler initialized");
        let (tx, rx) = flume::unbounded();
        // set up spawner
        SPAWNER.write().replace(spawner);
        let poll_thread_handle = thread::Builder::new()
            .name("poll_thread".to_string())
            .spawn(move || Self::poll_thread(rx))
            .expect("Failed to spawn poll_thread.");
        tracing::debug!("Spawned poll_thread");
        tracing::info!("Runtime startup complete.");
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
                tracing::error!("reactor wait error: {}, exit poll thread", e);
                break;
            }
        }
    }
    fn run(mut self) {
        tracing::info!("Runtime booted up, start execution...");
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
                    tracing::debug!("exit...");
                    break;
                }
            }
        }
        tracing::info!("Execution completed, shutting down...");
        // shutdown worker threads
        self.scheduler.shutdown();
        // shutdown poll thread
        self.poll_thread_notifier
            .send(())
            .expect("Failed to send notify");
        self.poll_thread_handle
            .join()
            .expect("Failed to join poll thread");
        tracing::info!("Runtime shutdown complete.")
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

pub fn spawn<F>(future: F)
where
    F: Future<Output = io::Result<()>> + Send + 'static,
{
    if let Some(spawner) = SPAWNER.read().deref() {
        spawner.spawn(future);
    }
}

pub fn shutdown() {
    if let Some(spawner) = SPAWNER.read().deref() {
        spawner.shutdown();
    }
}
