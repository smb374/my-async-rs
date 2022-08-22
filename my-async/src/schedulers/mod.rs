pub mod hybrid;
pub mod round_robin;
pub mod work_stealing;

use super::multi_thread::FutureIndex;

use std::{
    cell::Cell,
    future::Future,
    io,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    task::{Context, Poll, Waker},
};

use flume::{Receiver, Sender};
use futures_lite::future::FutureExt;
use once_cell::sync::{Lazy, OnceCell};
use parking_lot::{Mutex, RwLock};
use rustc_hash::FxHashMap;
use sharded_slab::Slab;

use crate::multi_thread::FUTURE_POOL;

static JOIN_HANDLE_MAP: Lazy<RwLock<FxHashMap<usize, Waker>>> =
    Lazy::new(|| RwLock::new(FxHashMap::default()));
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

pub enum ScheduleMessage {
    Schedule(FutureIndex),
    Reschedule(FutureIndex),
    Shutdown,
}

pub struct Spawner {
    tx: Sender<ScheduleMessage>,
}

pub trait Scheduler {
    fn init(size: usize) -> (Spawner, Self);
    fn schedule(&mut self, index: FutureIndex);
    fn reschedule(&mut self, index: FutureIndex);
    fn shutdown(self);
    fn receiver(&self) -> &Receiver<ScheduleMessage>;
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
    pub fn new(tx: Sender<ScheduleMessage>) -> Self {
        Self { tx }
    }
    pub fn spawn<F>(&self, future: F)
    where
        F: Future<Output = io::Result<()>> + Send + 'static,
    {
        let key = FUTURE_POOL
            .create_with(|seat| {
                seat.future.get_mut().replace(future.boxed());
            })
            .unwrap();
        let budget_index = BUDGET_SLAB
            .insert(AtomicUsize::new(DEFAULT_BUDGET))
            .unwrap();
        self.tx
            .send(ScheduleMessage::Schedule(FutureIndex {
                key,
                budget_index,
                sleep_count: 0,
            }))
            .expect("Failed to send message");
    }
    fn reschedule(&self, index: FutureIndex) {
        self.tx
            .send(ScheduleMessage::Reschedule(index))
            .expect("Failed to send message");
    }
    pub fn spawn_with_handle<F>(&self, future: F, is_block: bool) -> JoinHandle<F::Output>
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
        self.tx
            .send(ScheduleMessage::Schedule(FutureIndex {
                key,
                budget_index,
                sleep_count: 0,
            }))
            .expect("Failed to send message");
        JoinHandle {
            spawn_id: key,
            registered: AtomicBool::new(false),
            inner: result_arc,
        }
    }
    pub fn shutdown(&self) {
        self.tx
            .send(ScheduleMessage::Shutdown)
            .expect("Failed to send message");
    }
}

pub(super) fn init_spawner(spawner: Spawner) {
    SPAWNER.get_or_init(move || spawner);
}

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

pub fn shutdown() {
    let spawner = SPAWNER.get().unwrap();
    spawner.shutdown();
}

pub struct JoinHandle<T> {
    spawn_id: usize,
    registered: AtomicBool,
    inner: Arc<Mutex<Option<T>>>,
}

pub struct FutureJoin<'a, T> {
    handle: &'a JoinHandle<T>,
}

impl<T> JoinHandle<T> {
    pub fn join(&self) -> FutureJoin<'_, T> {
        FutureJoin { handle: self }
    }
    // `None`: Future not yet complete
    // `Some(val)`: Future completed with return value `val`.
    // Here we can simply use take since no one else will access after successful `try_join`
    pub fn try_join(&self) -> Option<T> {
        let mut guard = self.inner.lock();
        guard.take()
    }
    fn register_waker(&self, waker: Waker) {
        JOIN_HANDLE_MAP.write().insert(self.spawn_id, waker);
        self.registered.store(true, Ordering::Relaxed);
    }
    fn deregister_waker(&self) {
        JOIN_HANDLE_MAP.write().remove(&self.spawn_id);
        self.registered.store(false, Ordering::Relaxed);
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
    if let Some(waker) = JOIN_HANDLE_MAP.read().get(&index) {
        waker.wake_by_ref();
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
