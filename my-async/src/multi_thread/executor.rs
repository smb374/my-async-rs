use crate::{
    reactor::{Reactor, POLL_WAKER},
    schedulers::{self, shutdown, ScheduleMessage, Scheduler, Spawner},
};

use std::{future::Future, io, panic, process, thread, time::Duration};

use claim::assert_some;
use flume::Receiver;
use sharded_slab::Pool;

use super::BoxedFuture;

/// Executor that can run futures.
pub struct Executor<S: Scheduler> {
    scheduler: S,
    schedule_message_receiver: Receiver<ScheduleMessage>,
    future_pool: Pool<BoxedFuture>,
}

impl<S: Scheduler> Default for Executor<S> {
    fn default() -> Executor<S> {
        Executor::new()
    }
}

impl<S: Scheduler> Executor<S> {
    /// Create an executor instance.
    ///
    /// The function will setup the global spawner for [`spawn()`],
    /// and an IO polling thread for event notification.
    ///
    /// You should call this before [`spawn()`] or it won't have any effect.
    pub fn new() -> Self {
        let cpus = num_cpus::get();
        assert!(
            cpus > 0,
            "Failed to detect core number of current cpu, panic!"
        );
        let size = if cpus > 2 { cpus - 2 } else { cpus };
        let (tx, rx) = flume::unbounded();
        let scheduler = S::init(size);
        log::debug!("Scheduler initialized");
        // set up spawner
        schedulers::init_spawner(Spawner::new(tx));
        // set panic hook
        let default_hook = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            default_hook(info);
            log::error!("Runtime panics: {}", info);
            log::error!("Shutting down runtime with exit code 1...");
            shutdown();
            process::exit(1);
        }));
        log::info!("Runtime startup complete.");
        Self {
            scheduler,
            schedule_message_receiver: rx,
            future_pool: Pool::new(),
        }
    }
    fn poll_thread() {
        let mut reactor = Reactor::default();
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
        thread::scope(|s| {
            log::debug!("Spawn threads under scope...");
            let poll_thread_handle = thread::Builder::new()
                .name("poll_thread".to_string())
                .spawn_scoped(s, || Self::poll_thread())
                .expect("Failed to spawn poll_thread.");
            log::debug!("Spawned poll_thread");
            self.scheduler.setup_workers(s);
            log::info!("Runtime booted up, start execution...");
            loop {
                match self.schedule_message_receiver.recv() {
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
                        poll_thread_handle
                            .join()
                            .expect("Failed to join poll thread");
                        log::debug!("poll thread shutdown completed.");
                    }
                }
                None => log::error!("POLL_WAKER not set when trying to join poll thread!"),
            }
            log::info!("Runtime shutdown complete.");
        });
    }
    /// Blocks on a single future.
    ///
    /// The executor will continue to run until this future finishes.
    ///
    /// You can imagine that this will execute the "main" future function
    /// to complete before the executor shutdown.
    ///
    /// Note that it requires the future and its return type to be [`Send`] and `'static`.
    pub fn block_on<F>(self, future: F) -> F::Output
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let handle = schedulers::spawn_with_handle(future, true);
        log::info!("Start blocking...");
        self.run();
        log::debug!("Waiting result...");
        // The blocked on future finished executing, the result should be `Some(val)`
        let result = assert_some!(
            handle.try_join(),
            "The blocked future should produce a return value before the execution ends."
        );
        result
    }
}
