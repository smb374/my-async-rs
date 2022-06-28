pub mod round_robin;
pub mod work_stealing;

use std::{
    io,
    ops::DerefMut,
    sync::{Arc, Weak},
    task::{Context, Poll},
    time::{SystemTime, UNIX_EPOCH},
};

use crossbeam::channel::{self, Receiver, Sender};
use futures_lite::future::{Boxed, Future, FutureExt};
use futures_task::{waker_ref, ArcWake};
use parking_lot::Mutex;

pub enum ScheduleMessage {
    Schedule(BoxedFuture),
    Reschedule(Weak<Task>),
    Shutdown,
}

pub struct Spawner {
    tx: Sender<ScheduleMessage>,
}

pub trait Scheduler {
    fn init(size: usize) -> (Spawner, Self);
    fn schedule(&mut self, future: BoxedFuture);
    fn reschedule(&mut self, task: Weak<Task>);
    fn shutdown(self);
    fn receiver(&self) -> &Receiver<ScheduleMessage>;
}

pub type BoxedFuture = Mutex<Option<Boxed<io::Result<()>>>>;
pub type WrappedTaskSender = Arc<Mutex<Option<Sender<Weak<Task>>>>>;

pub struct Broadcast<T> {
    channels: Vec<Sender<T>>,
}

pub struct Task {
    future: BoxedFuture,
    tx: WrappedTaskSender,
}

impl<T: Send + Clone> Broadcast<T> {
    pub fn new() -> Self {
        Self {
            channels: Vec::with_capacity(num_cpus::get()),
        }
    }
    pub fn subscribe(&mut self) -> Receiver<T> {
        let (tx, rx) = channel::unbounded();
        self.channels.push(tx);
        rx
    }
    pub fn broadcast(&self, message: T) -> Result<(), channel::SendError<T>> {
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
        let boxed = Mutex::new(Some(future.boxed()));
        self.tx
            .send(ScheduleMessage::Schedule(boxed))
            .expect("Failed to send message");
    }
    pub fn shutdown(&self) {
        self.tx
            .send(ScheduleMessage::Shutdown)
            .expect("Failed to send message");
    }
}

impl Task {
    pub fn run(self: &Arc<Self>) {
        let mut guard = self.future.lock();
        let future_slot = guard.deref_mut();
        // run *ONCE*
        if let Some(mut fut) = future_slot.take() {
            let waker = waker_ref(self);
            let cx = &mut Context::from_waker(&waker);
            match fut.as_mut().poll(cx) {
                Poll::Ready(r) => {
                    if let Err(e) = r {
                        tracing::error!("Error occurred when executing future: {}", e);
                    }
                }
                Poll::Pending => {
                    *future_slot = Some(fut);
                }
            };
        }
    }
    pub fn replace_tx(&self, tx: Sender<Weak<Task>>) {
        let mut guard = self.tx.lock();
        guard.deref_mut().replace(tx);
    }
}

impl ArcWake for Task {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        let weak = Arc::downgrade(arc_self);
        let guard = arc_self.tx.lock();
        guard
            .as_ref()
            .expect("task's tx should be assigned when scheduled at the first time")
            .send(weak)
            .expect("Too many message queued!");
    }
}

pub(super) fn get_unix_time() -> u128 {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time earlier than UNIX_EPOCH!");
    dur.as_nanos()
}
