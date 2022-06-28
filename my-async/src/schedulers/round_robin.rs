use super::{BoxedFuture, ScheduleMessage, Scheduler, Spawner, Task};
use crate::get_unix_time;

use std::{
    sync::{Arc, Weak},
    thread::{self, JoinHandle},
};

use crossbeam::channel::{self, Receiver, Select, Sender};
use parking_lot::Mutex;
use rustc_hash::FxHashMap;

enum Message {
    Close,
}

pub struct RoundRobinScheduler {
    size: usize,
    task_hold: FxHashMap<u128, Arc<Task>>,
    current_index: usize,
    threads: Vec<(WorkerInfo, JoinHandle<()>)>,
    rx: Receiver<ScheduleMessage>,
}

struct WorkerInfo {
    task_tx: Sender<Weak<Task>>,
    tx: Sender<Message>,
}

pub struct Worker {
    _idx: usize,
    task_rx: Receiver<Weak<Task>>,
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
                let handle = thread::spawn(move || {
                    let worker = Worker { _idx, task_rx, rx };
                    worker.run();
                });
                (WorkerInfo { task_tx, tx }, handle)
            })
            .collect();
        let scheduler = Self {
            size,
            task_hold: FxHashMap::default(),
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
    fn schedule(&mut self, future: BoxedFuture) {
        let index = self.round();
        let task_tx = &self.threads[index].0.task_tx;
        let task = Arc::new(Task {
            future,
            tx: Arc::new(Mutex::new(Some(task_tx.clone()))),
        });
        let weak = Arc::downgrade(&task);
        self.task_hold.insert(get_unix_time(), task);
        task_tx.send(weak).expect("Failed to send message");
    }
    fn reschedule(&mut self, task: Weak<Task>) {
        let index = self.round();
        let task_tx = &self.threads[index].0.task_tx;
        task_tx.send(task).expect("Failed to send message");
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
                    if let Ok(weak) = oper.recv(&self.task_rx) {
                        if let Some(task) = weak.upgrade() {
                            task.run();
                        } else {
                            tracing::error!(
                                "Failed to upgrade the weak reference of current task!"
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
