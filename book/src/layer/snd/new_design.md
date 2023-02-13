# UPDATE: Redesigned Executor logic
In the past few weeks before this page came out I have been reworking the logic
of the `Executor` that currently:
1. Make the message handler become an event handler of the `Reactor` that is triggered when a notification
   to the `Reactor` was made. `Reactor::wait` has a new argument that accepts this handler.
2. Use scoped thread before further experiment that is related to lifetime is started as
   `std::thread:scope` will create lifetime bounds that will come in handy. The `Scheduler`
   trait also have a new `setup_workers()` because of this.

As a result, we can have one more worker that can execute the tasks.
## Code
### Reactor Change
The `Reactor` will handle the event associated with `POLL_WAKE_TOKEN`,
which is the token of the notifier, with the `notify_handler` passed as an argument:
```rust
impl Reactor {
    // ...
    pub fn wait<F>(&mut self, timeout: Option<Duration>, mut notify_handler: F) -> io::Result<bool>
    where
        F: FnMut() -> bool,
    {
        let mut result: bool = false;
        self.poll.poll(&mut self.events, timeout)?;
        if !self.events.is_empty() {
            log::debug!("Start process events.");
            for e in self.events.iter() {
                // We will now accept a handler in this function
                // that will be triggered with POLL_WAKE_TOKEN
                // which is related to a notifier.
                if e.token() == POLL_WAKE_TOKEN {
                    result = notify_handler();
                }
                let idx = e.token().0;
                let waker_processed = process_waker(idx, |guard| {
                    if let Some(w) = guard.take() {
                        w.wake_by_ref();
                    }
                });

                if !waker_processed {
                    self.extra_wakeups.push(idx);
                }
            }
        }
        Ok(result)
    }
    // ...
}
```
The returned boolean result will determine whether to shutdown the runtime.

### Spawner Change
For this design to work, we need to make all the spawning function defined by `Spawner`
trigger the notifier after the message is sent:
```rust
// This function will be called at the end of each of the spawn function that Spawner defines.
pub(crate) fn notify_reactor() {
    // assert_some! is used since it's impossible since
    // the POLL_WAKER is already initialized before
    // this function is called.
    // Also it's much more clear than using .unwrap()
    assert_some!(POLL_WAKER.get()).wake().unwrap();
}
```

### Executor Change
The structure changes a lot here.
1. The workers are initialized when `Executor::run()` is called, not when
the `Executor` is initialized.
2. The message handling code become an independent function and use non-blocking
receive to prevent blocking on channel empty:
```rust
fn message_handler(&mut self) -> bool {
    let mut result = false;
    loop {
        match self.schedule_message_receiver.try_recv() {
            // continously schedule tasks
            Ok(msg) => match msg {
                ScheduleMessage::Schedule(future) => self.scheduler.schedule(future),
                ScheduleMessage::Reschedule(task) => self.scheduler.reschedule(task),
                ScheduleMessage::Shutdown => {
                    result = true;
                    break;
                }
            },
            Err(e) => match e {
                TryRecvError::Empty => {
                    break;
                }
                TryRecvError::Disconnected => {
                    log::debug!("exit...");
                    result = true;
                    break;
                }
            },
        }
    }
    result
}
```
3. The `run()` function is greatly simplified since we don't need to handle thread joining
thanks to scoped thread and merged IO polling:
```rust
fn run(mut self, reactor: &mut Reactor) -> io::Result<()> {
    // 'env lifetime
    thread::scope(|s| -> io::Result<()> {
        // 'scope lifetime
        log::debug!("Spawn threads under scope...");
        // Setup workers in scope and before reactor loop.
        // All the threads will be joined at the end of this scope.
        self.scheduler.setup_workers(s);
        log::info!("Runtime booted up, start execution...");
        loop {
            // Check events that weren't handled by anyone before.
            reactor.check_extra_wakeups();
            // Wait with a timeout of 100 ms.
            // You can use None here as any command from Spawner
            // will wake the Reactor.
            //
            // The wait will return io::Result<bool>,
            // the boolean will determine whether to shutdown the runtime.
            // Other IO errors will also trigger shutdown of the runtime,
            // except Interrupted and WouldBlock as we simply retry.
            match reactor.wait(Some(Duration::from_millis(100)), || self.message_handler()) {
                Ok(false) => {}
                Ok(true) => break,
                Err(e) => match e.kind() {
                    // Simply retry when these two happened.
                    io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock => {}
                    _ => {
                        log::error!("Reactor wait error: {}, shutting down...", e);
                        break;
                    }
                },
            }
        }
        log::info!("Execution completed, shutting down...");
        // shutdown worker threads
        self.scheduler.shutdown();
        log::info!("Runtime shutdown complete.");
        Ok(())
    })?;
    Ok(())
}
```
