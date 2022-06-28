use super::reactor;
use super::schedulers::{ScheduleMessage, Scheduler, Spawner};

use std::{
    io,
    ops::Deref,
    sync::Arc,
    thread::{self, JoinHandle},
    time::Duration,
};

use crossbeam::channel::{self, Receiver, Sender, TryRecvError};
use futures_lite::prelude::*;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use tracing::metadata::LevelFilter;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

static SPAWNER: Lazy<Mutex<Option<Spawner>>> = Lazy::new(|| Mutex::new(None));

pub struct Executor<S: Scheduler> {
    scheduler: S,
    poll_thread_notifier: Sender<()>,
    poll_thread_handle: JoinHandle<()>,
}

impl<S: Scheduler> Executor<S> {
    pub fn new() -> Self {
        let format = fmt::layer()
            .with_level(true)
            .with_target(false)
            .with_thread_ids(false)
            .with_thread_names(true);
        let filter = EnvFilter::builder()
            .with_default_directive(LevelFilter::WARN.into())
            .from_env_lossy();
        tracing_subscriber::registry()
            .with(format)
            .with(filter)
            .init();
        let cpus = num_cpus::get();
        let size = if cpus == 0 { 1 } else { cpus - 2 };
        let (spawner, scheduler) = S::init(size);
        tracing::debug!("Scheduler initialized");
        let (tx, rx) = channel::unbounded();
        // set up spawner
        SPAWNER.lock().replace(spawner);
        let poll_thread_handle = thread::Builder::new()
            .name("poll_thread".to_string())
            .spawn(move || Self::poll_thread(rx))
            .expect("Failed to spawn poll_thread.");
        tracing::debug!("Spawned poll_thread");
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
            if let Err(e) = reactor.wait(Some(Duration::from_millis(100))) {
                tracing::error!("reactor wait error: {}, exit poll thread", e);
                break;
            }
        }
    }
    fn run(mut self) {
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
        tracing::info!("Execution completed, shutdown");
        // shutdown worker threads
        self.scheduler.shutdown();
        // shutdown poll thread
        self.poll_thread_notifier
            .send(())
            .expect("Failed to send notify");
        self.poll_thread_handle
            .join()
            .expect("Failed to join poll thread");
    }
    pub fn block_on<F, T>(self, future: F) -> Option<T>
    where
        T: Send + 'static,
        F: Future<Output = T> + Send + 'static,
    {
        let result_arc: Arc<Mutex<Option<T>>> = Arc::new(Mutex::new(None));
        let weak = Arc::downgrade(&result_arc);
        spawn(async move {
            let result = future.await;
            if let Some(arc) = weak.upgrade() {
                arc.lock().replace(result);
            } else {
                tracing::error!("Result storing arc being dropped for unknown reason!!!");
            }
            shutdown();
            Ok(())
        });
        self.run();
        let mut guard = result_arc.lock();
        guard.take()
    }
}

pub fn spawn<F>(future: F)
where
    F: Future<Output = io::Result<()>> + Send + 'static,
{
    if let Some(spawner) = SPAWNER.lock().deref() {
        spawner.spawn(future);
    }
}

pub fn spawn_with_result<F, T>(future: F) -> Option<T>
where
    T: Send + 'static,
    F: Future<Output = T> + Send + 'static,
{
    let result_arc: Arc<Mutex<Option<T>>> = Arc::new(Mutex::new(None));
    let weak = Arc::downgrade(&result_arc);
    spawn(async move {
        let result = future.await;
        if let Some(arc) = weak.upgrade() {
            arc.lock().replace(result);
        } else {
            tracing::error!("Result storing arc being dropped for unknown reason!!!");
        }
        Ok(())
    });
    let mut guard = result_arc.lock();
    guard.take()
}

pub fn shutdown() {
    if let Some(spawner) = SPAWNER.lock().deref() {
        spawner.shutdown();
    }
}
