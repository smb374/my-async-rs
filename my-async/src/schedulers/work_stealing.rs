use super::{BoxedFuture, Broadcast, ScheduleMessage, Scheduler, Spawner, Task};
use crate::get_unix_time;

use std::{
    sync::{Arc, Weak},
    thread,
};

use crossbeam::{
    channel::{self, Receiver, Select, Sender},
    deque::{Injector, Stealer, Worker},
    sync::WaitGroup,
};
use parking_lot::Mutex;
use rustc_hash::FxHashMap;

pub struct WorkStealingScheduler {
    _size: usize,
    // this `HashMap` holds the ownership of tasks that spawns in this scheduler
    // using unix timestamp for its key for further access.
    // TODO: check if we can use `HashSet` to replace `HashMap` here because it may not be necessary to access the task by key afterall.
    task_hold: FxHashMap<u128, Arc<Task>>,
    injector: Arc<Injector<Weak<Task>>>,
    _stealers: Vec<Stealer<Weak<Task>>>,
    wait_group: WaitGroup,
    notifier: Broadcast<Message>,
    rx: Receiver<ScheduleMessage>,
}

struct TaskRunner {
    _idx: usize,
    worker: Worker<Weak<Task>>,
    injector: Arc<Injector<Weak<Task>>>,
    stealers: Arc<[Stealer<Weak<Task>>]>,
    rx: Receiver<Message>,
    task_tx: Sender<Weak<Task>>,
    task_rx: Receiver<Weak<Task>>,
}

#[derive(Clone)]
enum Message {
    HaveTasks,
    Close,
}

impl WorkStealingScheduler {
    fn new(size: usize) -> (Spawner, Self) {
        let injector: Arc<Injector<Weak<Task>>> = Arc::new(Injector::new());
        let mut _stealers: Vec<Stealer<Weak<Task>>> = Vec::new();
        let stealers_arc: Arc<[Stealer<Weak<Task>>]> = Arc::from(_stealers.as_slice());
        let (tx, rx) = channel::unbounded();
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
                    let (task_tx, task_rx) = channel::unbounded();
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
                    drop(wg);
                })
                .expect("Failed to spawn worker");
        }
        let scheduler = Self {
            _size: size,
            task_hold: FxHashMap::default(),
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
    fn schedule(&mut self, future: BoxedFuture) {
        let task = Arc::new(Task {
            id: get_unix_time(),
            future,
            tx: Mutex::new(None),
        });
        let weak = Arc::downgrade(&task);
        self.task_hold.insert(task.id, task);
        self.injector.push(weak);
        self.notifier
            .broadcast(Message::HaveTasks)
            .expect("Failed to send message");
    }
    fn reschedule(&mut self, task: Weak<Task>) {
        self.injector.push(task);
        self.notifier
            .broadcast(Message::HaveTasks)
            .expect("Failed to send message");
    }
    fn shutdown(self) {
        self.notifier
            .broadcast(Message::Close)
            .expect("Failed to send message");
        self.wait_group.wait();
    }
    fn receiver(&self) -> &Receiver<ScheduleMessage> {
        &self.rx
    }
}

impl TaskRunner {
    fn run(&self) {
        let mut select = Select::new();
        let task_index = select.recv(&self.task_rx);
        let rx_index = select.recv(&self.rx);
        'outer: loop {
            match self.worker.pop() {
                Some(weak) => {
                    if let Some(task) = weak.upgrade() {
                        task.replace_tx(self.task_tx.clone());
                        task.run();
                    } else {
                        tracing::error!("Failed to upgrade the weak reference of current task!");
                    }
                }
                None => {
                    tracing::debug!("Start collecting tasks...");
                    let mut wakeup_count = 0;
                    // First push in all the woke up Task, non-blocking.
                    tracing::debug!("Collecting wokeups");
                    loop {
                        match self.task_rx.try_recv() {
                            Ok(weak) => {
                                wakeup_count += 1;
                                self.worker.push(weak);
                            }
                            Err(channel::TryRecvError::Empty) => break,
                            Err(channel::TryRecvError::Disconnected) => break 'outer,
                        }
                    }
                    if wakeup_count > 0 {
                        continue;
                    }
                    // If we are starving, start stealing.
                    if let Some(weak) = self.steal_task() {
                        self.worker.push(weak);
                        continue;
                    }
                    // Finally, wait for a single wakeup task or broadcast signal from scheduler
                    let oprv = select.select();
                    match oprv.index() {
                        i if i == task_index => match oprv.recv(&self.task_rx) {
                            Ok(weak) => self.worker.push(weak),
                            Err(_) => break,
                        },
                        i if i == rx_index => match oprv.recv(&self.rx) {
                            Ok(Message::HaveTasks) => continue,
                            Ok(Message::Close) | Err(_) => break 'outer,
                        },
                        _ => unreachable!(),
                    }
                }
            }
        }
    }

    fn steal_task(&self) -> Option<Weak<Task>> {
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
