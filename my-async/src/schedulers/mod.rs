pub mod hybrid;
pub mod round_robin;
pub mod work_stealing;

use super::multi_thread::FutureIndex;

use std::{future::Future, io};

use flume::{Receiver, Sender};
use futures_lite::future::FutureExt;

use crate::multi_thread::FUTURE_POOL;

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
pub struct Broadcast<T> {
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
        self.tx
            .send(ScheduleMessage::Schedule(FutureIndex {
                key,
                sleep_count: 0,
            }))
            .expect("Failed to send message");
    }
    pub fn shutdown(&self) {
        self.tx
            .send(ScheduleMessage::Shutdown)
            .expect("Failed to send message");
    }
}
