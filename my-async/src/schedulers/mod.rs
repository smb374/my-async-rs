pub mod round_robin;
pub mod work_stealing;

use std::{
    io,
    task::{Context, Poll},
};

use flume::{Receiver, Sender};
use futures_lite::future::{Boxed, Future, FutureExt};
use parking_lot::Mutex;
use sharded_slab::Clear;
use waker_fn::waker_fn;

use crate::multi_thread::FUTURE_POOL;

// Single access from `Task` itself only, using `Mutex`
pub type WrappedTaskSender = Option<Sender<FutureIndex>>;
pub type FutureIndex = usize;

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

// Single access from `Task` itself only, using `Mutex`
pub struct BoxedFuture(Mutex<Option<Boxed<io::Result<()>>>>);

impl Default for BoxedFuture {
    fn default() -> Self {
        BoxedFuture(Mutex::new(None))
    }
}

impl Clear for BoxedFuture {
    fn clear(&mut self) {
        self.0.get_mut().clear();
    }
}

impl BoxedFuture {
    pub fn run(&self, index: FutureIndex, tx: Sender<FutureIndex>) -> bool {
        let mut guard = self.0.lock();
        // run *ONCE*
        if let Some(fut) = guard.as_mut() {
            let waker = waker_fn(move || {
                tx.send(index).expect("Too many message queued!");
            });
            let cx = &mut Context::from_waker(&waker);
            match fut.as_mut().poll(cx) {
                Poll::Ready(r) => {
                    if let Err(e) = r {
                        tracing::error!("Error occurred when executing future: {}", e);
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

// TODO: check if there is a better way to broadcast message instead of this naive implementation.
pub struct Broadcast<T> {
    channels: Vec<Sender<T>>,
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
        let index = FUTURE_POOL
            .create_with(|seat| {
                seat.0.get_mut().replace(future.boxed());
            })
            .unwrap();
        self.tx
            .send(ScheduleMessage::Schedule(index))
            .expect("Failed to send message");
    }
    pub fn shutdown(&self) {
        self.tx
            .send(ScheduleMessage::Shutdown)
            .expect("Failed to send message");
    }
}
