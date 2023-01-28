//! Round-Robin fashion Scheduler
//!
//! This module implements a scheduler that will schedule the tasks
//! in a round robin fashion.
//!
//! Tasks will always be resend to the original worker it was scheduled
//! unless it's been rescheduled.
use super::{FutureIndex, Scheduler};

use std::thread;

// use crossbeam::channel::{self, Receiver, Select, Sender};
use flume::{Receiver, Selector, Sender};

enum Message {
    Close,
}

pub struct RoundRobinScheduler {
    size: usize,
    current_index: usize,
    threads: Vec<WorkerInfo>,
}

struct WorkerInfo {
    task_tx: Sender<FutureIndex>,
    tx: Sender<Message>,
}

struct Worker {
    _idx: usize,
    task_tx: Sender<FutureIndex>,
    task_rx: Receiver<FutureIndex>,
    rx: Receiver<Message>,
}

impl RoundRobinScheduler {
    fn new(size: usize) -> Self {
        let threads = Vec::with_capacity(size);
        Self {
            size,
            current_index: 0,
            threads,
        }
    }
    fn round(&mut self) -> usize {
        let r = self.current_index;
        self.current_index = (self.current_index + 1) % self.size;
        r
    }
}

impl Scheduler for RoundRobinScheduler {
    fn init(size: usize) -> Self {
        Self::new(size)
    }
    fn schedule(&mut self, index: FutureIndex) {
        let worker_index = self.round();
        let task_tx = &self.threads[worker_index].task_tx;
        task_tx.send(index).expect("Failed to send message");
    }
    fn reschedule(&mut self, index: FutureIndex) {
        let worker_index = self.round();
        let task_tx = &self.threads[worker_index].task_tx;
        task_tx.send(index).expect("Failed to send message");
    }
    fn shutdown(self) {
        for info in self.threads {
            let tx = info.tx;
            tx.send(Message::Close).expect("Failed to send message");
        }
    }
    fn setup_workers<'s, 'e: 's>(&mut self, s: &'s thread::Scope<'s, 'e>) {
        for idx in 0..self.size {
            let (tx, rx) = flume::unbounded();
            let (task_tx, task_rx) = flume::unbounded();
            let tx_clone = task_tx.clone();
            s.spawn(move || {
                let worker = Worker {
                    _idx: idx,
                    task_tx: tx_clone,
                    task_rx,
                    rx,
                };
                worker.run();
            });
            self.threads[idx] = WorkerInfo { task_tx, tx };
        }
    }
}

impl Worker {
    fn run(&self) {
        loop {
            let exit_loop = Selector::new()
                .recv(&self.task_rx, |result| match result {
                    Ok(index) => {
                        super::process_future(index, &self.task_tx);
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
