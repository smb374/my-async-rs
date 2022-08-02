use super::{wake_join_handle, Broadcast, FutureIndex, ScheduleMessage, Scheduler, Spawner};
use crate::multi_thread::FUTURE_POOL;

#[allow(unused_imports)]
use std::{
    hash::BuildHasherDefault,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
};

use crossbeam_deque::{Injector, Steal, Stealer, Worker};
use crossbeam_utils::sync::WaitGroup;
use flume::{Receiver, Selector, Sender, TryRecvError};
// use once_cell::sync::OnceCell;
use priority_queue::priority_queue::PriorityQueue;
use rustc_hash::FxHasher;

// static TRANS_STATUS: OnceCell<Vec<AtomicBool>> = OnceCell::new();

#[derive(Clone)]
enum Message {
    HaveTasks,
    Close,
}

struct TaskQueue {
    cold: Worker<FutureIndex>,
    hot: PriorityQueue<FutureIndex, usize, BuildHasherDefault<FxHasher>>,
}

// A prioritized work stealing scheduler with a hybrid task queue.
pub struct HybridScheduler {
    wait_group: WaitGroup,
    injector: Arc<Injector<FutureIndex>>,
    _stealers: Vec<Stealer<FutureIndex>>,
    // channels
    schedule_message_receiver: Receiver<ScheduleMessage>,
    notifier: Broadcast<Message>,
}

struct TaskRunner {
    idx: usize,
    queue: TaskQueue,
    injector: Arc<Injector<FutureIndex>>,
    stealers: Arc<[Stealer<FutureIndex>]>,
    // channels
    task_wakeup_sender: Sender<FutureIndex>,
    task_wakeup_receiver: Receiver<FutureIndex>,
    notify_receiver: Receiver<Message>,
}

impl HybridScheduler {
    pub fn new(size: usize) -> (Spawner, Self) {
        let (schedule_message_sender, schedule_message_receiver) = flume::unbounded();
        let injector: Arc<Injector<FutureIndex>> = Arc::new(Injector::new());
        let mut _stealers = Vec::with_capacity(size);
        let stealers_arc: Arc<[Stealer<FutureIndex>]> = Arc::from(_stealers.as_slice());
        // TRANS_STATUS.get_or_init(|| {
        //     let mut trans = Vec::new();
        //     for _ in 0..size {
        //         trans.push(AtomicBool::new(false));
        //     }
        //     trans
        // });
        let mut notifier = Broadcast::new();
        let wait_group = WaitGroup::new();
        for idx in 0..size {
            let worker = Worker::new_lifo();
            let wg = wait_group.clone();
            let notify_receiver = notifier.subscribe();
            _stealers.push(worker.stealer());
            let ic = Arc::clone(&injector);
            let sc = Arc::clone(&stealers_arc);
            thread::Builder::new()
                .name(format!("hybrid_worker_{}", idx))
                .spawn(move || {
                    let (task_wakeup_sender, task_wakeup_receiver) = flume::unbounded();
                    let mut runner = TaskRunner {
                        idx,
                        queue: TaskQueue {
                            cold: worker,
                            hot: PriorityQueue::with_capacity_and_default_hasher(65536),
                        },
                        injector: ic,
                        stealers: sc,
                        task_wakeup_sender,
                        task_wakeup_receiver,
                        notify_receiver,
                    };
                    runner.run();
                    log::debug!("Runner shutdown.");
                    drop(wg);
                })
                .expect("Failed to spawn worker");
        }
        let spawner = Spawner::new(schedule_message_sender);
        let scheduler = Self {
            wait_group,
            injector,
            _stealers,
            schedule_message_receiver,
            notifier,
        };
        (spawner, scheduler)
    }
}

impl Scheduler for HybridScheduler {
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
            .expect("Faild to send shutdown notify");
        log::debug!("Waiting runners to shutdown...");
        self.wait_group.wait();
        log::debug!("Shutdown complete.");
    }
    fn receiver(&self) -> &Receiver<ScheduleMessage> {
        &self.schedule_message_receiver
    }
}

impl TaskRunner {
    fn run(&mut self) {
        'outer: loop {
            if let Some((index, _)) = self.queue.hot.pop() {
                Self::process_future(index, &self.task_wakeup_sender);
            } else {
                log::debug!("Start collecting tasks...");
                // Step 1. cold -> hot
                log::debug!("Cold to hot");
                let mut push = false;
                // ensure that nothing is stolen from cold queue when we transfer indexes to hot queue.
                // TRANS_STATUS.get().unwrap()[self.idx]
                //     .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
                //     .expect("Failed to exchange!");
                if !self.queue.cold.is_empty() {
                    push = true;
                    // cold -> hot
                    while let Some(index) = self.queue.cold.pop() {
                        self.queue.hot.push(index, index.sleep_count);
                    }
                }
                // TRANS_STATUS.get().unwrap()[self.idx]
                //     .compare_exchange(true, false, Ordering::Relaxed, Ordering::Relaxed)
                //     .expect("Failed to exchange!");
                if push {
                    continue;
                }
                // Step 2. pull from wakeups
                log::debug!("Collecting wokeups...");
                let mut recv_count = 0;
                loop {
                    match self.task_wakeup_receiver.try_recv() {
                        Ok(index) => {
                            self.queue.cold.push(index);
                            recv_count += 1;
                        }
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => break 'outer,
                    }
                }
                if recv_count > 0 {
                    // we aren't starving, no need to steal.
                    continue;
                }
                // Step 3. steal
                log::debug!("Try stealing tasks from other runners...");
                if let Some(index) = self.injector.steal().success() {
                    self.queue.cold.push(index);
                    continue;
                }
                if let Some(index) = self.steal_task() {
                    self.queue.cold.push(index);
                    continue;
                }
                // Step 4. wait
                log::debug!("Runner park.");
                let exit_loop = Selector::new()
                    .recv(&self.task_wakeup_receiver, |result| match result {
                        Ok(index) => {
                            self.queue.cold.push(index);
                            false
                        }
                        Err(_) => true,
                    })
                    .recv(&self.notify_receiver, |result| match result {
                        Ok(Message::HaveTasks) => {
                            if let Some(index) = self.injector.steal().success() {
                                self.queue.cold.push(index);
                            }
                            false
                        }
                        Ok(Message::Close) | Err(_) => true,
                    })
                    .wait();
                if exit_loop {
                    break 'outer;
                }
            }
        }
    }

    fn process_future(index: FutureIndex, tx: &Sender<FutureIndex>) {
        if let Some(boxed) = FUTURE_POOL.get(index.key) {
            let finished = boxed.run(&index, tx.clone());
            if finished {
                wake_join_handle(index.key);
                if !FUTURE_POOL.clear(index.key) {
                    log::error!(
                        "Failed to remove completed future with index = {} from pool.",
                        index.key
                    );
                }
            }
        } else {
            log::error!("Future with index = {} is not in pool.", index.key);
        }
    }

    fn steal_task(&self) -> Option<FutureIndex> {
        for (i, s) in self.stealers.iter().enumerate() {
            // let trans = &TRANS_STATUS.get().unwrap()[self.idx];
            if i == self.idx {
                continue;
            } else if let Steal::Success(index) = s.steal() {
                return Some(index);
            }
        }
        None
    }
}
