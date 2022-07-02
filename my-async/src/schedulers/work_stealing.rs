use super::{Broadcast, FutureIndex, ScheduleMessage, Scheduler, Spawner};
use crate::multi_thread::FUTURE_POOL;

use std::{sync::Arc, thread};

use crossbeam_deque::{Injector, Stealer, Worker};
use crossbeam_utils::sync::WaitGroup;
use flume::{Receiver, Selector, Sender, TryRecvError};

pub struct WorkStealingScheduler {
    _size: usize,
    injector: Arc<Injector<FutureIndex>>,
    _stealers: Vec<Stealer<FutureIndex>>,
    wait_group: WaitGroup,
    notifier: Broadcast<Message>,
    rx: Receiver<ScheduleMessage>,
}

struct TaskRunner {
    _idx: usize,
    worker: Worker<FutureIndex>,
    injector: Arc<Injector<FutureIndex>>,
    stealers: Arc<[Stealer<FutureIndex>]>,
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
        let injector: Arc<Injector<FutureIndex>> = Arc::new(Injector::new());
        let mut _stealers: Vec<Stealer<FutureIndex>> = Vec::new();
        let stealers_arc: Arc<[Stealer<FutureIndex>]> = Arc::from(_stealers.as_slice());
        let (tx, rx) = flume::unbounded();
        let mut notifier = Broadcast::new();
        let spawner = Spawner::new(tx);
        let wait_group = WaitGroup::new();
        for _idx in 0..size {
            let worker = Worker::new_fifo();
            _stealers.push(worker.stealer());
            let ic = Arc::clone(&injector);
            let sc = Arc::clone(&stealers_arc);
            let wg = wait_group.clone();
            let rc = notifier.subscribe();
            thread::Builder::new()
                .name(format!("work_stealing_worker_{}", _idx))
                .spawn(move || {
                    let (task_tx, task_rx) = flume::unbounded();
                    let runner = TaskRunner {
                        _idx,
                        worker,
                        injector: ic,
                        stealers: sc,
                        rx: rc,
                        task_tx,
                        task_rx,
                    };
                    runner.run();
                    tracing::debug!("Runner shutdown.");
                    drop(wg);
                })
                .expect("Failed to spawn worker");
        }
        let scheduler = Self {
            _size: size,
            injector,
            _stealers,
            wait_group,
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
        self.injector.push(index);
        self.notifier
            .broadcast(Message::HaveTasks)
            .expect("Failed to send message");
    }
    fn reschedule(&mut self, index: FutureIndex) {
        self.injector.push(index);
        self.notifier
            .broadcast(Message::HaveTasks)
            .expect("Failed to send message");
    }
    fn shutdown(self) {
        self.notifier
            .broadcast(Message::Close)
            .expect("Failed to send message");
        tracing::debug!("Waiting runners to shutdown...");
        self.wait_group.wait();
        tracing::debug!("Shutdown complete.");
    }
    fn receiver(&self) -> &Receiver<ScheduleMessage> {
        &self.rx
    }
}

impl TaskRunner {
    fn run(&self) {
        'outer: loop {
            match self.worker.pop() {
                Some(index) => {
                    if let Some(boxed) = FUTURE_POOL.get(index) {
                        let finished = boxed.run(index, self.task_tx.clone());
                        if finished {
                            if !FUTURE_POOL.clear(index) {
                                tracing::error!(
                                    "Failed to remove completed future with index = {} from pool.",
                                    index
                                );
                            }
                        }
                    } else {
                        tracing::error!("Future with index = {} is not in pool.", index);
                    }
                }
                None => {
                    tracing::debug!("Start collecting tasks...");
                    let mut wakeup_count = 0;
                    // First push in all the woke up Task, non-blocking.
                    tracing::debug!("Collecting wokeups...");
                    loop {
                        match self.task_rx.try_recv() {
                            Ok(index) => {
                                wakeup_count += 1;
                                self.worker.push(index);
                            }
                            Err(TryRecvError::Empty) => break,
                            Err(TryRecvError::Disconnected) => break 'outer,
                        }
                    }
                    if wakeup_count > 0 {
                        continue;
                    }
                    // If we are starving, start stealing.
                    tracing::debug!("Try stealing tasks from other runners...");
                    if let Some(index) = self.steal_task() {
                        self.worker.push(index);
                        continue;
                    }
                    // Finally, wait for a single wakeup task or broadcast signal from scheduler
                    tracing::debug!("Runner park.");
                    let exit_loop = Selector::new()
                        .recv(&self.task_rx, |result| match result {
                            Ok(index) => {
                                self.worker.push(index);
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
    }

    fn steal_task(&self) -> Option<FutureIndex> {
        // will generate *ONE* task at a time
        std::iter::repeat_with(|| {
            self.injector
                .steal_batch_and_pop(&self.worker)
                .or_else(|| self.stealers.iter().map(|s| s.steal()).collect())
        })
        .find(|s| !s.is_retry())
        .and_then(|s| s.success())
    }
}
