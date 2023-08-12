# Passing messages

According to the previous section, we have two commands to handle:

1. spawn
2. shutdown

The corresponding messages are defined:

```rust
// in src/schedulers/mod.rs:
pub enum ScheduleMessage {
    Schedule(FutureIndex),
    Reschedule(FutureIndex),
    Shutdown,
}
```

Where `Schedule` is to handle spawn, and `Shutdown` is to handle shutdown.

The `Reschedule` message is an internal message that is used to re-queue a task, usually used when
a worker's queue is full that the worker can put excessive tasks back to global queue for others to take.
For more details, see [The procedure of task scheduling](../trd/schedule_procedure.md).

With the message defined, we can code the message handle loop of `Executor`:

```rust
// Executor::run() code
fn run(mut self) {
    loop {
        // The sender half will send message.
        match self.scheduler.receiver().recv() { // Blocking receive
            Ok(msg) => match msg {
                ScheduleMessage::Schedule(future) => // Scheduler schedule future,
                ScheduleMessage::Reschedule(task) => // Scheduler reschedule task,
                ScheduleMessage::Shutdown => break,
            },
            Err(_) => { // sender disconnected
                log::debug!("exit...");
                break;
            }
        }
    }
    // Shutdown procedures...
}
```

The `Executor` will receive messages until:

1. Sender half of the channel is disconnected.
2. Got `ScheduleMessage::Shutdown`.

In the subsections, we'll look at:

1. The sender half.
2. The message payload.
