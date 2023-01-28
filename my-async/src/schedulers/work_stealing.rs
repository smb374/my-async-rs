//! Work-Stealing Scheduler
//!
//! This module implements a scheduler that uses the work-stealing strategy.
//! The tasks in local queue can be stolen by other worker that is not busy.
use super::{Broadcast, FutureIndex, Scheduler};
use crate::schedulers::reschedule;

use std::{sync::Arc, thread};

use concurrent_ringbuf::{Ringbuf, Stealer};
use crossbeam_utils::sync::WaitGroup;
use flume::{Receiver, Selector, Sender, TryRecvError};

pub struct WorkStealingScheduler {
    size: usize,
    stealers: Vec<Stealer<FutureIndex>>,
    wait_group: WaitGroup,
    // channels
    inject_sender: Sender<FutureIndex>,
    inject_receiver: Receiver<FutureIndex>,
    notifier: Broadcast<Message>,
}

struct TaskRunner {
    _idx: usize,
    worker: Ringbuf<FutureIndex>,
    stealers: Arc<[Stealer<FutureIndex>]>,
    // channels
    inject_receiver: Receiver<FutureIndex>,
    rx: Receiver<Message>,
    task_tx: Sender<FutureIndex>,
    task_rx: Receiver<FutureIndex>,
}

#[derive(Clone)]
enum Message {
    HaveTasks,
    Close,
}

impl WorkStealingScheduler {
    fn new(size: usize) -> Self {
        let stealers: Vec<Stealer<FutureIndex>> = Vec::new();
        let (inject_sender, inject_receiver) = flume::unbounded();
        let notifier = Broadcast::new();
        let wait_group = WaitGroup::new();
        for _idx in 0..size {
        }
        Self {
            size,
            stealers,
            wait_group,
            inject_sender,
            inject_receiver,
            notifier,
        }
    }
}

impl Scheduler for WorkStealingScheduler {
    fn init(size: usize) -> Self {
        Self::new(size)
    }
    fn schedule(&mut self, index: FutureIndex) {
        self.inject_sender
            .send(index)
            .expect("Failed to send message");
        self.notifier
            .broadcast(Message::HaveTasks)
            .expect("Failed to send message");
    }
    fn reschedule(&mut self, index: FutureIndex) {
        self.inject_sender
            .send(index)
            .expect("Failed to send message");
        self.notifier
            .broadcast(Message::HaveTasks)
            .expect("Failed to send message");
    }
    fn shutdown(self) {
        self.notifier
            .broadcast(Message::Close)
            .expect("Failed to send message");
        log::debug!("Waiting runners to shutdown...");
        self.wait_group.wait();
        log::debug!("Shutdown complete.");
    }
    fn setup_workers<'s, 'e: 's>(&mut self, s: &'s thread::Scope<'s, 'e>) {
        let stealers_arc: Arc<[Stealer<FutureIndex>]> = Arc::from(self.stealers.as_slice());
        for idx in 0..self.size {
            let worker = Ringbuf::new(4096);
            self.stealers.push(worker.stealer());
            let ic = self.inject_receiver.clone();
            let sc = Arc::clone(&stealers_arc);
            let wg = self.wait_group.clone();
            let rc = self.notifier.subscribe();
            thread::Builder::new()
                .name(format!("work_stealing_worker_{}", idx))
                .spawn_scoped(s, move || {
                    let (task_tx, task_rx) = flume::unbounded();
                    let runner = TaskRunner {
                        _idx: idx,
                        worker,
                        stealers: sc,
                        inject_receiver: ic,
                        rx: rc,
                        task_tx,
                        task_rx,
                    };
                    runner.run();
                    log::debug!("Runner shutdown.");
                    drop(wg);
                })
                .expect("Failed to spawn worker");
        }
    }
}

impl TaskRunner {
    fn run(&self) {
        'outer: loop {
            if !self.worker.is_empty() {
                while let Some(index) = self.worker.pop() {
                    super::process_future(index, &self.task_tx);
                }
            } else {
                log::debug!("Start collecting tasks...");
                let mut wakeup_count = 0;
                // First push in all the woke up Task, non-blocking.
                log::debug!("Collecting wokeups...");
                loop {
                    match self.task_rx.try_recv() {
                        Ok(index) => {
                            wakeup_count += 1;
                            if let Err(index) = self.worker.push(index) {
                                reschedule(index);
                            }
                        }
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => break 'outer,
                    }
                }
                if wakeup_count > 0 {
                    continue;
                }
                // If we are starving, start stealing.
                log::debug!("Try stealing tasks from other runners...");
                if let Ok(index) = self.inject_receiver.try_recv() {
                    if let Err(index) = self.worker.push(index) {
                        reschedule(index);
                    }
                    continue;
                }
                if let Some(index) = self.steal_others() {
                    if let Err(index) = self.worker.push(index) {
                        reschedule(index);
                    }
                    continue;
                }
                // Finally, wait for a single wakeup task or broadcast signal from scheduler
                log::debug!("Runner park.");
                let exit_loop = Selector::new()
                    .recv(&self.task_rx, |result| match result {
                        Ok(index) => {
                            if let Err(index) = self.worker.push(index) {
                                reschedule(index);
                            }
                            false
                        }
                        Err(_) => true,
                    })
                    .recv(&self.rx, |result| match result {
                        Ok(Message::HaveTasks) => false,
                        Ok(Message::Close) | Err(_) => true,
                    })
                    .wait();
                if exit_loop {
                    break 'outer;
                }
            }
        }
    }

    fn steal_others(&self) -> Option<FutureIndex> {
        self.stealers
            .iter()
            .map(|s| s.steal())
            .find(|s| s.is_success())
            .and_then(|s| s.success())
    }
}
