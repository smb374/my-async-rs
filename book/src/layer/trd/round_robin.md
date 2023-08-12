# Round Robin

This scheduler uses a Round-Robin style scheduling strategy.

## Structs

```rust
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

struct Worker {
    _idx: usize,
    task_tx: Sender<FutureIndex>,
    task_rx: Receiver<FutureIndex>,
    rx: Receiver<Message>,
}
```

- The injector of this scheduler is replaced by an array of senders as we use Round-Robin.
- Channels used:
  - `task_tx`, `task_rx`: Channel for tasks. `task_tx` is copied to the waker and the threads array.
    - Because of this, woke up tasks and new tasks shared the same channel and the channel can be used as the task queue.
  - Worker `tx`, `rx`: Shutdown notifier.
  - Scheduler `rx`: Commands receiver.

## Scheduler implementation

```rust
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
```

The `schedule` and `reschedule` simply get an index by `round`, and use the channel corresponding to the index to schedule the task.
`shutdown` will simply use `tx` of every worker to send shutdown message.

### Initialization

```rust
fn new(size: usize) -> (Spawner, Self) {
    let (tx, rx) = flume::unbounded();
    let spawner = Spawner::new(tx);
    let threads: Vec<(WorkerInfo, JoinHandle<()>)> = (0..size)
        .map(|_idx| {
            let (tx, rx) = flume::unbounded();
            let (task_tx, task_rx) = flume::unbounded();
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
```

Simply create the global spawner and the worker threads with channels introduced previously.

## Worker run loop

```rust
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
```

Simply wait both `task_rx` and `rx`. If `task_rx` has tasks, receive it and run it.
If receive shutdown notification from `rx`, break the loop.
