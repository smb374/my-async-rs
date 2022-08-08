use crate::schedulers;

use super::reactor;
use super::schedulers::{ScheduleMessage, Scheduler};

use std::{
    future::Future,
    hash::Hash,
    io,
    task::{Context, Poll},
    thread::{self, JoinHandle},
    time::Duration,
};
use std::{panic, process};

use claim::assert_some;
use flume::{Receiver, Sender, TryRecvError};
use futures_lite::future::Boxed;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use sharded_slab::{Clear, Pool};
use waker_fn::waker_fn;

pub use schedulers::{shutdown, spawn};

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

pub(crate) struct BoxedFuture {
    pub(crate) future: Mutex<Option<Boxed<io::Result<()>>>>,
}

impl Default for BoxedFuture {
    fn default() -> Self {
        BoxedFuture {
            future: Mutex::new(None),
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

// global future allocation pool.
pub(crate) static FUTURE_POOL: Lazy<Pool<BoxedFuture>> = Lazy::new(Pool::new);

pub struct Executor<S: Scheduler> {
    scheduler: S,
    poll_thread_notify_sender: Sender<()>,
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
        let (tx, rx) = flume::bounded(1);
        log::debug!("Scheduler initialized");
        // set up spawner
        schedulers::init_spawner(spawner);
        let poll_thread_handle = thread::Builder::new()
            .name("poll_thread".to_string())
            .spawn(move || Self::poll_thread(rx))
            .expect("Failed to spawn poll_thread.");
        log::debug!("Spawned poll_thread");
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
            poll_thread_notify_sender: tx,
            poll_thread_handle,
        }
    }
    fn poll_thread(poll_thread_notify_receiver: Receiver<()>) {
        let mut reactor = reactor::Reactor::default();
        loop {
            match poll_thread_notify_receiver.try_recv() {
                Err(TryRecvError::Empty) => {}
                _ => break,
            }
            // check if wakeups that is not used immediately is needed now.
            reactor.check_extra_wakeups();
            match reactor.wait(Some(Duration::from_millis(100))) {
                Ok(()) => continue,
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
        self.poll_thread_notify_sender.send(()).unwrap();
        self.poll_thread_handle
            .join()
            .expect("Failed to join poll thread.");
        log::info!("Runtime shutdown complete.")
    }
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
