use super::{Broadcast, FutureIndex, ScheduleMessage, Scheduler, Spawner};
use crate::schedulers::reschedule;

use std::{sync::Arc, thread};

use concurrent_ringbuf::{Ringbuf, Stealer};
use crossbeam_utils::sync::WaitGroup;
use flume::{Receiver, Selector, Sender, TryRecvError};

pub struct WorkStealingScheduler {
    _size: usize,
    _stealers: Vec<Stealer<FutureIndex>>,
    wait_group: WaitGroup,
    handles: Vec<thread::JoinHandle<()>>,
    // channels
    inject_sender: Sender<FutureIndex>,
    notifier: Broadcast<Message>,
    rx: Receiver<ScheduleMessage>,
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
    fn new(size: usize) -> (Spawner, Self) {
        let mut _stealers: Vec<Stealer<FutureIndex>> = Vec::new();
        let stealers_arc: Arc<[Stealer<FutureIndex>]> = Arc::from(_stealers.as_slice());
        let (inject_sender, inject_receiver) = flume::unbounded();
        let mut handles = Vec::with_capacity(size);
        let (tx, rx) = flume::unbounded();
        let mut notifier = Broadcast::new();
        let spawner = Spawner::new(tx);
        let wait_group = WaitGroup::new();
        for _idx in 0..size {
            let worker = Ringbuf::new(4096);
            _stealers.push(worker.stealer());
            let ic = inject_receiver.clone();
            let sc = Arc::clone(&stealers_arc);
            let wg = wait_group.clone();
            let rc = notifier.subscribe();
            let handle = thread::Builder::new()
                .name(format!("work_stealing_worker_{}", _idx))
                .spawn(move || {
                    let (task_tx, task_rx) = flume::unbounded();
                    let runner = TaskRunner {
                        _idx,
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
            handles.push(handle);
        }
        let scheduler = Self {
            _size: size,
            _stealers,
            wait_group,
            handles,
            inject_sender,
            notifier,
            rx,
        };
        (spawner, scheduler)
    }
}

impl Scheduler for WorkStealingScheduler {
    fn init(size: usize) -> (Spawner, Self) {
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
        self.handles.into_iter().for_each(|h| h.join().unwrap());
        log::debug!("Shutdown complete.");
    }
    fn receiver(&self) -> &Receiver<ScheduleMessage> {
        &self.rx
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
