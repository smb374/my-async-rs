use crate::{
    reactor::Reactor,
    schedulers::{self, shutdown, ScheduleMessage, Scheduler, Spawner},
};

use core::{future::Future, time::Duration};
use std::{io, panic, process, thread};

use claims::{assert_ok, assert_some};
use flume::{Receiver, TryRecvError};

/// Executor that can run futures.
pub struct Executor<S: Scheduler> {
    scheduler: S,
    schedule_message_receiver: Receiver<ScheduleMessage>,
}

impl<S: Scheduler> Default for Executor<S> {
    fn default() -> Executor<S> {
        Executor::new()
    }
}

impl<S: Scheduler> Executor<S> {
    /// Create an executor instance.
    ///
    /// The function will setup the global spawner for [`spawn()`][a],
    /// and an IO polling thread for event notification.
    ///
    /// You should call this before [`spawn()`][a] or it won't have any effect.
    ///
    /// [a]: crate::schedulers::spawn()
    pub fn new() -> Self {
        let cpus = num_cpus::get();
        assert!(
            cpus > 0,
            "Failed to detect core number of current cpu, panic!"
        );
        let size = if cpus > 1 { cpus - 1 } else { cpus };
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
        }
    }
    fn run(mut self, reactor: &mut Reactor) -> io::Result<()> {
        // 'env
        thread::scope(|s| -> io::Result<()> {
            // 'scope
            log::debug!("Spawn threads under scope...");
            self.scheduler.setup_workers(s);
            log::info!("Runtime booted up, start execution...");
            loop {
                reactor.check_extra_wakeups();
                match reactor.wait(Some(Duration::from_millis(100)), || self.message_handler()) {
                    Ok(false) => {}
                    Ok(true) => break,
                    Err(e) => match e.kind() {
                        io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock => {}
                        _ => {
                            log::error!("Reactor wait error: {}, shutting down...", e);
                            break;
                        }
                    },
                }
            }
            log::info!("Execution completed, shutting down...");
            // shutdown worker threads
            self.scheduler.shutdown();
            log::info!("Runtime shutdown complete.");
            Ok(())
        })?;
        Ok(())
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
        let mut reactor = Reactor::default();
        reactor.setup_registry();
        let handle = schedulers::spawn_with_handle(future, true);
        log::info!("Start blocking...");
        assert_ok!(self.run(&mut reactor));
        log::debug!("Waiting result...");
        // The blocked on future finished executing, the result should be `Some(val)`
        let result = assert_some!(
            handle.try_join(),
            "The blocked future should produce a return value before the execution ends."
        );
        result
    }
    fn message_handler(&mut self) -> bool {
        let mut result = false;
        loop {
            match self.schedule_message_receiver.try_recv() {
                // continously schedule tasks
                Ok(msg) => match msg {
                    ScheduleMessage::Schedule(future) => self.scheduler.schedule(future),
                    ScheduleMessage::Reschedule(task) => self.scheduler.reschedule(task),
                    ScheduleMessage::Shutdown => {
                        result = true;
                        break;
                    }
                },
                Err(e) => match e {
                    TryRecvError::Empty => {
                        break;
                    }
                    TryRecvError::Disconnected => {
                        log::debug!("exit...");
                        result = true;
                        break;
                    }
                },
            }
        }
        result
    }
}
