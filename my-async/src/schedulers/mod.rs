//! Scheduler trait an various default implementation.
//!
//! The module defines and implements various crutial parts
//! for [`multi_thread::Executor`][super::multi_thread::Executor]
//! to work:
//! * [`Scheduler`] trait: Defines the behaviour of a schduler should have
//! for the multi-threaded executor to work.
//! * [`JoinHandle`]: The future task handle for async and non-blocking joining a futture.
//! * [`FutureJoin`]: [`Future`] implementation for async join.
//! * [`Spawner`]: Multi-threaded future task spawner that is initialized during scheduler
//! initialization.
//! * [`spawn()`] and [`shutdown()`]: Multi-threaded version of the single-threaded version.
//! Need [`Spawner`] to be initialized to work.
//! * [`ScheduleMessage`]: Interthread message to communicate with [`Scheduler`].

pub mod hybrid;
pub mod round_robin;
pub mod work_stealing;

use crate::reactor::notify_reactor;

use super::multi_thread::{FutureIndex, FUTURE_POOL};

use core::{
    cell::Cell,
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    task::{Context, Poll, Waker},
};
use std::{sync::Arc, thread::Scope};

use flume::{Receiver, Sender};
use futures_lite::future::FutureExt;
use once_cell::sync::{Lazy, OnceCell};
use parking_lot::Mutex;
use sharded_slab::Slab;

static SPAWNER: OnceCell<Spawner> = OnceCell::new();
static BUDGET_SLAB: Lazy<Slab<AtomicUsize>> = Lazy::new(Slab::new);
const DEFAULT_BUDGET: usize = 128;
// thread local budget handle statics
thread_local! {
    pub(crate) static USING_BUDGET: Cell<bool> = Cell::new(false);
    static CURRENT_INDEX: Cell<usize> = Cell::new(usize::MAX);
    // cache to reduce atomic actions
    static BUDGET_CACHE: Cell<usize> = Cell::new(usize::MAX);
}

/// Interthread message to communicate with [`Scheduler`].
///
/// Currently only 3 variants:
/// 1. `Schedule`: Schedules a new future.
/// 2. `Reschedule`: Reschedule a future. Can be used when local worker queue is full.
/// 3. `Shutdown`: Shutdown message to shutdown the scheduler.
pub enum ScheduleMessage {
    /// Schedule a new future task.
    ///
    /// Because of the design of multi-threaded executor,
    /// the message itself contains some counter and the task's index.
    ///
    /// See [`FutureIndex`] for more information.
    Schedule(FutureIndex),
    /// Reschedule an existing future task.
    Reschedule(FutureIndex),
    /// Shutdown signal for scheduler.
    Shutdown,
}

/// Sender for sending [`ScheduleMessage`].
///
/// It can be used to schedule task, reschedule task, and shutdown scheduler.
pub struct Spawner {
    tx: Sender<ScheduleMessage>,
}

/// Defines the behaviour of a schduler should have for the multi-threaded executor to work.
///
/// Example schedulers are defined under [`round_robin`], [`work_stealing`], and [`hybrid`].
pub trait Scheduler {
    /// Initialize the [`Spawner`] and the [`Scheduler`] it self.
    ///
    /// It should initialize `size` threads as worker for concurrent future processing.
    fn init(size: usize) -> Self;
    /// Schedules incoming future tasks to workers.
    ///
    /// There's no restriction as long as it accepts a [`FutureIndex`]
    fn schedule(&mut self, index: FutureIndex);
    /// Reschedules incoming future tasks to workers.
    ///
    /// There's no restriction as long as it accepts a [`FutureIndex`]
    fn reschedule(&mut self, index: FutureIndex);
    /// Shutdown the scheduler.
    ///
    /// It should notify all worker to shutdown and join the worker threads.
    fn shutdown(self);
    fn setup_workers<'s, 'e: 's>(&mut self, s: &'s Scope<'s, 'e>);
}

// TODO: check if there is a better way to broadcast message instead of this naive implementation.
pub(crate) struct Broadcast<T> {
    channels: Vec<Sender<T>>,
}

impl<T: Send + Clone> Default for Broadcast<T> {
    fn default() -> Broadcast<T> {
        Broadcast::new()
    }
}

impl<T: Send + Clone> Broadcast<T> {
    pub fn new() -> Self {
        Self {
            channels: Vec::with_capacity(num_cpus::get()),
        }
    }
    pub fn subscribe(&mut self) -> Receiver<T> {
        let (tx, rx) = flume::unbounded();
        self.channels.push(tx);
        rx
    }
    pub fn broadcast(&self, message: T) -> Result<(), flume::SendError<T>> {
        self.channels
            .iter()
            .try_for_each(|tx| tx.send(message.clone()))?;
        Ok(())
    }
}

impl Spawner {
    /// Create a spawner using sender half.
    ///
    /// The receiving half is held by the scheduler and used in executor's main
    /// loop to receive [`ScheduleMessage`]s.
    pub fn new(tx: Sender<ScheduleMessage>) -> Self {
        Self { tx }
    }
    fn reschedule(&self, index: FutureIndex) {
        self.tx
            .send(ScheduleMessage::Reschedule(index))
            .expect("Failed to send message");
        notify_reactor();
    }
    /// Spawns a task with [`JoinHandle`]
    ///
    /// This is the default action used by [`spawn()`].
    pub fn spawn_with_handle<F>(&self, future: F, is_block: bool) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let (index, handle) = alloc_future(future, is_block);
        self.tx
            .send(ScheduleMessage::Schedule(index))
            .expect("Failed to send message");
        notify_reactor();
        handle
    }
    /// Send shutdown signal to scheduler.
    pub fn shutdown(&self) {
        self.tx
            .send(ScheduleMessage::Shutdown)
            .expect("Failed to send message");
        notify_reactor();
    }
}

pub(super) fn init_spawner(spawner: Spawner) {
    SPAWNER.get_or_init(move || spawner);
}

/// Spawns a future task and return a [`JoinHandle`].
///
/// This functions spawns a future task and returns a [`JoinHandle`] for joining the task.
pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    spawn_with_handle(future, false)
}

pub(super) fn spawn_with_handle<F>(future: F, is_block: bool) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let spawner = SPAWNER.get().unwrap();
    spawner.spawn_with_handle(future, is_block)
}

pub(crate) fn reschedule(index: FutureIndex) {
    let spawner = SPAWNER.get().unwrap();
    spawner.reschedule(index);
}

/// Notifies the [`Scheduler`] to shutdown.
///
/// Used when early exit and blocked future finishes.
pub fn shutdown() {
    let spawner = SPAWNER.get().unwrap();
    spawner.shutdown();
}

/// Join handle for a future task.
///
/// The handle can do two stuff: async [`join()`][JoinHandle::join()]
/// and non-blocking [`try_join()`][JoinHandle::try_join()].
pub struct JoinHandle<T> {
    spawn_id: usize,
    registered: AtomicBool,
    inner: Arc<Mutex<Option<T>>>,
}

/// Struct for [`JoinHandle::join()`]'s [`Future`] implementation.
pub struct FutureJoin<'a, T> {
    handle: &'a JoinHandle<T>,
}

impl<T> JoinHandle<T> {
    /// Normal async join for a future task.
    ///
    /// To join a future task in a non-async function or non-blocking wait,
    /// use [`try_join()`][JoinHandle::try_join()] instead.
    pub fn join(&self) -> FutureJoin<'_, T> {
        FutureJoin { handle: self }
    }
    /// Non-blocking join for a future task.
    ///
    /// This function can be used in non-async envoironment since the implementation
    /// doesn't use any [`Future`] related code. It can also be used when quick checking since
    /// `.await` is not required.
    ///
    /// If the join is success, it will return `Some(val)`, other wise `None` indicating that the
    /// task isn't finished yet.
    // `None`: Future not yet complete
    // `Some(val)`: Future completed with return value `val`.
    // Here we can simply use take since no one else will access after successful `try_join`
    pub fn try_join(&self) -> Option<T> {
        let mut guard = self.inner.lock();
        guard.take()
    }
    fn register_waker(&self, waker: Waker) {
        if let Some(entry) = FUTURE_POOL.get(self.spawn_id) {
            entry.join_waker.lock().replace(waker);
            self.registered.store(true, Ordering::Relaxed);
        }
    }
    fn deregister_waker(&self) {
        if let Some(entry) = FUTURE_POOL.get(self.spawn_id) {
            entry.join_waker.lock().take();
            self.registered.store(false, Ordering::Relaxed);
        }
    }
}

impl<'a, T> Future for FutureJoin<'a, T> {
    type Output = T;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.handle;
        let mut guard = me.inner.lock();
        match guard.take() {
            Some(val) => {
                if me.registered.load(Ordering::Relaxed) {
                    me.deregister_waker();
                }
                Poll::Ready(val)
            }
            None => {
                if !me.registered.load(Ordering::Relaxed) {
                    // spawned future can use it's own id to wake JoinHandle.
                    me.register_waker(cx.waker().clone());
                }
                Poll::Pending
            }
        }
    }
}

// Called when a spawned future finishes running
// If the waker is registered, we wake it up
// Otherwise, the JoinHandle hasn't request join.
pub(super) fn wake_join_handle(index: usize) {
    if let Some(entry) = FUTURE_POOL.get(index) {
        let guard = entry.join_waker.lock();
        if let Some(waker) = guard.as_ref() {
            waker.wake_by_ref();
        }
    }
}

pub(crate) fn budget_update(index: &FutureIndex) -> Option<usize> {
    let mut result = None;
    let budget_index = index.budget_index;
    let current_budget = match BUDGET_SLAB.get(budget_index) {
        Some(b) => b.load(Ordering::Relaxed),
        None => {
            let idx = BUDGET_SLAB
                .insert(AtomicUsize::new(DEFAULT_BUDGET))
                .expect("Slab is full!!!");
            result.replace(idx);
            DEFAULT_BUDGET
        }
    };
    let old_index = CURRENT_INDEX.with(|idx| idx.replace(budget_index));
    let old_budget = BUDGET_CACHE.with(|b| b.replace(current_budget));
    if old_index != usize::MAX && old_budget != usize::MAX {
        if let Some(b) = BUDGET_SLAB.get(old_index) {
            b.store(old_budget, Ordering::Relaxed);
        }
    }
    result
}

pub(crate) fn poll_with_budget<T, U>(fut: &mut T, cx: &mut Context<'_>) -> Poll<U>
where
    T: FutureExt<Output = U> + ?Sized + Unpin,
{
    USING_BUDGET.with(|x| x.replace(true));
    BUDGET_CACHE.with(|budget| {
        let val = budget.get();
        // if budget is zero, reschedule it by immediately wake the waker then return Poll::Pending (yield_now)
        if val == 0 {
            cx.waker().wake_by_ref();
            budget.set(DEFAULT_BUDGET);
            return Poll::Pending;
        }
        match fut.poll(cx) {
            Poll::Ready(x) => {
                // budget decreases when ready
                budget.set(val - 1);
                Poll::Ready(x)
            }
            Poll::Pending => Poll::Pending,
        }
    })
}

pub(crate) fn process_future(mut index: FutureIndex, tx: &Sender<FutureIndex>) {
    if let Some(boxed) = FUTURE_POOL.get(index.key) {
        USING_BUDGET.with(|ub| {
            if ub.get() {
                if let Some(idx) = budget_update(&index) {
                    index.budget_index = idx;
                }
                ub.replace(false);
            }
        });
        let finished = boxed.run(&index, tx.clone());
        if finished {
            wake_join_handle(index.key);
            if !FUTURE_POOL.clear(index.key) {
                log::error!(
                    "Failed to remove completed future with index = {} from pool.",
                    index.key
                );
            }
        }
    } else {
        log::error!("Future with index = {} is not in pool.", index.key);
    }
}

pub(crate) fn alloc_future<F>(future: F, is_block: bool) -> (FutureIndex, JoinHandle<F::Output>)
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let result_arc: Arc<Mutex<Option<F::Output>>> = Arc::new(Mutex::new(None));
    let clone = result_arc.clone();
    let spawn_fut = async move {
        let output = future.await;
        let mut guard = clone.lock();
        guard.replace(output);
        if is_block {
            log::info!("Shutting down...");
            shutdown();
        }
        Ok(())
    };
    let key = FUTURE_POOL
        .create_with(|seat| {
            seat.future.get_mut().replace(spawn_fut.boxed());
        })
        .unwrap();
    let budget_index = BUDGET_SLAB
        .insert(AtomicUsize::new(DEFAULT_BUDGET))
        .unwrap();
    let handle = JoinHandle {
        spawn_id: key,
        registered: AtomicBool::new(false),
        inner: result_arc,
    };
    let index = FutureIndex {
        key,
        budget_index,
        sleep_count: 0,
    };
    (index, handle)
}
