# Work Stealing

This scheduler implements the Work-Stealing strategy.

## Structs

```rust
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
```

- The injector is a channel used in SPMC condition.
- The injector here will not perform any scheduling strategy (first comer takes first), as we use Work-Stealing strategy.
- Channels used:
  - `inject_sender`, `inject_receiver`: Injector.
  - `notifier`, Worker `rx`: Broadcast `Message` to workers.
    - Either shutdown or wake the workers up to get new tasks
  - Worker `task_tx`, `task_rx`: Channel for waking up tasks. `task_tx` is copied to the waker.
- Worker `worker` ring buffer is designed to have the Work-Stealing function.
  - It's capable of `push`, `pop`, and create a stealer for other workers to steal task from it.
  - The stealers are created on scheduler init and uses a `Arc` guarded slice for access.
- The `wait_group` is used to synchronize each Worker at shutdown phase.

## Scheduler implementation

```rust
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
```

- `schedule`, `reschedule`: First push the task to be scheduled into the injector channel, then use the notifier to broadcast `Message::HaveTasks` to workers.
- `shutdown`: First use `notifier` to broadcast `Message::Close`. The use `wait_group` to wait all the threads run to the point before exit, and then we join all the threads.

### Initialization

```rust
fn new(size: usize) -> (Spawner, Self) {
    // 1.
    let mut _stealers: Vec<Stealer<FutureIndex>> = Vec::new();
    let stealers_arc: Arc<[Stealer<FutureIndex>]> = Arc::from(_stealers.as_slice());
    let (inject_sender, inject_receiver) = flume::unbounded();
    let mut handles = Vec::with_capacity(size);
    let (tx, rx) = flume::unbounded();
    let mut notifier = Broadcast::new();
    let spawner = Spawner::new(tx);
    let wait_group = WaitGroup::new();
    // 2.
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
```

1. Create global spawner, `wait_group`, `notifier`, injector channel, stealer array, and handlers array.
2. Create worker threads with a Work-Stealing ringbuf bounded with a size of 4096.

## Worker run loop

```rust
impl TaskRunner {
    fn run(&self) {
        'outer: loop {
            // 1.
            if !self.worker.is_empty() {
                while let Some(index) = self.worker.pop() {
                    super::process_future(index, &self.task_tx);
                }
            } else {
                log::debug!("Start collecting tasks...");
                // 2.
                let mut wakeup_count = 0;
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
                // 3.
                log::debug!("Try stealing tasks from other runners...");
                if let Ok(index) = self.inject_receiver.try_recv() {
                    if let Err(index) = self.worker.push(index) {
                        reschedule(index);
                    }
                    continue;
                }
                // 4.
                if let Some(index) = self.steal_others() {
                    if let Err(index) = self.worker.push(index) {
                        reschedule(index);
                    }
                    continue;
                }
                // Finally, wait for a single wakeup task or broadcast signal from scheduler
                log::debug!("Runner park.");
                // 5.
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
```

1. If `worker` is not empty, pop and process a task.
2. Non-blocking receive all tasks that is waked up.
   - If `wake_count > 0`, continue the loop.
3. Non-blocking receive a task that is queued in the injector.
   - If success, continue the loop.
4. Try to steal a task using the stealers recorded in the `stealers` array.
   - If success, continue the loop.
5. Block receive 1 woke up task or notifications that is `HaveTasks` or `Close`.
   - `Close` indicates that it's time to shut down, break run loop.
   - `HaveTasks` means there tasks scheduled/rescheduled. Continue the loop and run through step 3.

The `steal_others` make use of Rust's lazy iterator behavior, that it won't steal over 1 task, which is the first one that successes.
