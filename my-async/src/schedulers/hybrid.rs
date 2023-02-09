//! Hybrid Scheduler
//!
//! This module implements a [`HybridScheduler`], the `Hybrid` means
//! the hybrid queue used in this scheduler.
//!
//! Basically, the hybrid queue worked as:
//! * The new coming tasks or waked tasks will be placed in the `cold` queue, which is a ringbuffer that can be work-stealed.
//! * The `hot` queue is a local priority queue that can orders the task based on its priority.
//! * Whenever the `hot` queue is empty, load tasks from the `cold` queue.
//!
//! This implementation makes prioritized work-stealing strategy possible without implementing
//! a thread-safe priority queue with a complex synchronization scheme plus loads of cache miss.

use super::{Broadcast, FutureIndex, Scheduler};
use crate::schedulers::reschedule;

use std::{hash::BuildHasherDefault, sync::Arc, thread};

use concurrent_ringbuf::{Ringbuf, Stealer};
use crossbeam_utils::sync::WaitGroup;
use flume::{Receiver, Selector, Sender, TryRecvError};
use priority_queue::priority_queue::PriorityQueue;
use rustc_hash::FxHasher;

#[derive(Clone)]
enum Message {
    HaveTasks,
    Close,
}

struct TaskQueue {
    cold: Ringbuf<FutureIndex>,
    hot: PriorityQueue<FutureIndex, usize, BuildHasherDefault<FxHasher>>,
}

/// A prioritized work stealing scheduler with a hybrid task queue.
///
/// See [module documentation](./index.html) for more information.
pub struct HybridScheduler {
    size: usize,
    wait_group: WaitGroup,
    stealers: Vec<Stealer<FutureIndex>>,
    // channels
    inject_sender: Sender<FutureIndex>,
    inject_receiver: Receiver<FutureIndex>,
    notifier: Broadcast<Message>,
}

struct TaskRunner {
    idx: usize,
    queue: TaskQueue,
    stealers: Arc<[Stealer<FutureIndex>]>,
    // channels
    inject_receiver: Receiver<FutureIndex>,
    task_wakeup_sender: Sender<FutureIndex>,
    task_wakeup_receiver: Receiver<FutureIndex>,
    notify_receiver: Receiver<Message>,
}

impl HybridScheduler {
    pub fn new(size: usize) -> Self {
        let (inject_sender, inject_receiver) = flume::unbounded();
        let stealers = Vec::with_capacity(size);
        let notifier = Broadcast::new();
        let wait_group = WaitGroup::new();
        Self {
            size,
            wait_group,
            stealers,
            inject_sender,
            inject_receiver,
            notifier,
        }
    }
}

impl Scheduler for HybridScheduler {
    fn init(size: usize) -> Self {
        Self::new(size)
    }
    fn schedule(&mut self, index: FutureIndex) {
        self.inject_sender.send(index).unwrap();
        self.notifier
            .broadcast(Message::HaveTasks)
            .expect("Failed to send message");
    }
    fn reschedule(&mut self, index: FutureIndex) {
        self.inject_sender.send(index).unwrap();
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
    fn setup_workers<'s, 'e: 's>(&mut self, s: &'s thread::Scope<'s, 'e>) {
        let stealers_arc: Arc<[Stealer<FutureIndex>]> = Arc::from(self.stealers.as_slice());
        for idx in 0..self.size {
            let rb = Ringbuf::new(4096);
            let wg = self.wait_group.clone();
            let notify_receiver = self.notifier.subscribe();
            self.stealers.push(rb.stealer());
            let ic = self.inject_receiver.clone();
            let sc = Arc::clone(&stealers_arc);
            thread::Builder::new()
                .name(format!("hybrid_worker_{idx}"))
                .spawn_scoped(s, move || {
                    let (task_wakeup_sender, task_wakeup_receiver) = flume::unbounded();
                    let mut runner = TaskRunner {
                        idx,
                        queue: TaskQueue {
                            cold: rb,
                            hot: PriorityQueue::with_capacity_and_default_hasher(65536),
                        },
                        stealers: sc,
                        inject_receiver: ic,
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
    }
}

impl TaskRunner {
    fn run(&mut self) {
        'outer: loop {
            if let Some((index, _)) = self.queue.hot.pop() {
                super::process_future(index, &self.task_wakeup_sender);
            } else {
                log::debug!("Start collecting tasks...");
                // Step 1. cold -> hot
                log::debug!("Cold to hot");
                let mut push = false;
                if !self.queue.cold.is_empty() {
                    push = true;
                    // cold -> hot
                    while let Some(index) = self.queue.cold.pop() {
                        self.queue.hot.push(index, index.sleep_count);
                    }
                }
                if push {
                    continue;
                }
                // Step 2. pull from wakeups
                log::debug!("Collecting wokeups...");
                let mut recv_count = 0;
                loop {
                    match self.task_wakeup_receiver.try_recv() {
                        Ok(index) => {
                            if let Err(index) = self.queue.cold.push(index) {
                                reschedule(index);
                            }
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
                if let Ok(index) = self.inject_receiver.try_recv() {
                    if let Err(index) = self.queue.cold.push(index) {
                        reschedule(index);
                    }
                    continue;
                }
                if let Some(index) = self.steal_task() {
                    if let Err(index) = self.queue.cold.push(index) {
                        reschedule(index);
                    }
                    continue;
                }
                // Step 4. wait
                log::debug!("Runner park.");
                let exit_loop = Selector::new()
                    .recv(&self.task_wakeup_receiver, |result| match result {
                        Ok(index) => {
                            if let Err(index) = self.queue.cold.push(index) {
                                reschedule(index);
                            }
                            false
                        }
                        Err(_) => true,
                    })
                    .recv(&self.notify_receiver, |result| match result {
                        Ok(Message::HaveTasks) => {
                            if let Ok(index) = self.inject_receiver.try_recv() {
                                if let Err(index) = self.queue.cold.push(index) {
                                    reschedule(index);
                                }
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

    fn steal_task(&self) -> Option<FutureIndex> {
        for (i, s) in self.stealers.iter().enumerate() {
            if i == self.idx {
                continue;
            } else if let Some(index) = s.steal().success() {
                return Some(index);
            }
        }
        None
    }
}
