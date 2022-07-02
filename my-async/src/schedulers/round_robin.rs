use super::{FutureIndex, ScheduleMessage, Scheduler, Spawner};
use crate::multi_thread::FUTURE_POOL;

use std::thread::{self, JoinHandle};

use crossbeam::channel::{self, Receiver, Select, Sender};

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
        let (tx, rx) = channel::unbounded();
        let spawner = Spawner::new(tx);
        let threads: Vec<(WorkerInfo, JoinHandle<()>)> = (0..size)
            .map(|_idx| {
                let (tx, rx) = channel::unbounded();
                let (task_tx, task_rx) = channel::unbounded();
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
        let mut select = Select::new();
        let task_index = select.recv(&self.task_rx);
        let rx_index = select.recv(&self.rx);
        loop {
            let oper = select.select();
            match oper.index() {
                i if i == task_index => {
                    if let Ok(index) = oper.recv(&self.task_rx) {
                        if let Some(boxed) = FUTURE_POOL.get(index) {
                            boxed.run(index, self.task_tx.clone());
                        } else {
                            tracing::error!(
                                "Future with index = {} disappeared in pool, check the runtime.",
                                index
                            );
                        }
                    } else {
                        break;
                    }
                }
                i if i == rx_index => match oper.recv(&self.rx) {
                    Ok(Message::Close) => break,
                    Err(e) => {
                        eprintln!("recv error: {}", e);
                        break;
                    }
                },
                _ => unreachable!(),
            }
        }
    }
}
