use super::{FutureIndex, ScheduleMessage, Scheduler, Spawner};
use crate::multi_thread::FUTURE_POOL;

use std::thread::{self, JoinHandle};

// use crossbeam::channel::{self, Receiver, Select, Sender};
use flume::{Receiver, Selector, Sender};

enum Message {
    Close,
}

pub struct RoundRobinScheduler {
    size: usize,
    current_index: usize,
    threads: Vec<(WorkerInfo, JoinHandle<()>)>,
    rx: Receiver<ScheduleMessage>,
}

struct WorkerInfo {
    task_tx: Sender<FutureIndex>,
    tx: Sender<Message>,
}

pub struct Worker {
    _idx: usize,
    task_tx: Sender<FutureIndex>,
    task_rx: Receiver<FutureIndex>,
    rx: Receiver<Message>,
}

impl RoundRobinScheduler {
    fn new(size: usize) -> (Spawner, Self) {
        let (tx, rx) = flume::unbounded();
        let spawner = Spawner::new(tx);
        let threads: Vec<(WorkerInfo, JoinHandle<()>)> = (0..size)
            .map(|_idx| {
                let (tx, rx) = flume::unbounded();
                let (task_tx, task_rx) = flume::unbounded();
                let tx_clone = task_tx.clone();
                let handle = thread::spawn(move || {
                    let worker = Worker {
                        _idx,
                        task_tx: tx_clone,
                        task_rx,
                        rx,
                    };
                    worker.run();
                });
                (WorkerInfo { task_tx, tx }, handle)
            })
            .collect();
        let scheduler = Self {
            size,
            current_index: 0,
            threads,
            rx,
        };
        (spawner, scheduler)
    }
    fn round(&mut self) -> usize {
        let r = self.current_index;
        self.current_index = (self.current_index + 1) % self.size;
        r
    }
}

impl Scheduler for RoundRobinScheduler {
    fn init(size: usize) -> (Spawner, Self) {
        Self::new(size)
    }
    fn schedule(&mut self, index: FutureIndex) {
        let worker_index = self.round();
        let task_tx = &self.threads[worker_index].0.task_tx;
        task_tx.send(index).expect("Failed to send message");
    }
    fn reschedule(&mut self, index: FutureIndex) {
        let worker_index = self.round();
        let task_tx = &self.threads[worker_index].0.task_tx;
        task_tx.send(index).expect("Failed to send message");
    }
    fn shutdown(self) {
        for (info, handle) in self.threads {
            let tx = info.tx;
            tx.send(Message::Close).expect("Failed to send message");
            let _ = handle.join();
        }
    }
    fn receiver(&self) -> &Receiver<ScheduleMessage> {
        &self.rx
    }
}

impl Worker {
    fn run(&self) {
        loop {
            let exit_loop = Selector::new()
                .recv(&self.task_rx, |result| match result {
                    Ok(index) => {
                        if let Some(boxed) = FUTURE_POOL.get(index.key) {
                            let finished = boxed.run(&index, self.task_tx.clone());
                            if finished && !FUTURE_POOL.clear(index.key) {
                                log::error!(
                                    "Failed to remove completed future with index = {} from pool.",
                                    index.key
                                );
                            }
                        } else {
                            log::error!("Future with index = {} is not in pool.", index.key);
                        }
                        false
                    }
                    Err(_) => true,
                })
                .recv(&self.rx, |result| match result {
                    Ok(Message::Close) => true,
                    Err(_) => true,
                })
                .wait();
            if exit_loop {
                break;
            }
        }
    }
}
