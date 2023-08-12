# Executor Main Loop
Inside the main loop, there are a few stages:
1. Pop and make a certain of progress of all futures in the waiting queue.
2. Try to receive to receive as much as futures that is waken up by the reactor.
3. Try to receive any message from the spawner.
4. Finally, wait for reactor to harvest events.

## Stage 1
```rust
let mut reactor = reactor::Reactor::default();
reactor.setup_registry();
'outer: loop {
    if let Some(index) = self.queue.pop_back() {
        FUTURE_POOL.with(|p| {
            if let Some(boxed) = p.get(index.key) {
                let finished = boxed.run(&index, self.task_tx.clone());
                if finished && !p.clear(index.key) {
                    log::error!(
                        "Failed to remove completed future with index = {} from pool.",
                        index.key
                    );
                }
            } else {
                log::error!("Future with index = {} is not in pool.", index.key);
            }
        });
    } else {
      // Other stages
    }
}
```

Here we first setup the reactor for later use, and start popping `FutureIndex`s from
waiting queue. The error handling here is simply log the errors to the logging facility
for maintaining a short code. The process can be addressed as:
1. Retrieve the `BoxedFuture` by the key of `FutureIndex`.
2. Use `BoxedFuture::run()` to make progress with the return value indicating id it's finished.
3. If it's finished, delete it from the global storage.

## Stage 2
```rust
'outer: loop {
    if let Some(index) = self.queue.pop_back() {
        // Stage 1
    } else {
        let mut wakeup_count = 0;
        loop {
            match self.task_rx.try_recv() {
                Ok(index) => {
                    wakeup_count += 1;
                    self.queue.push_front(index);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break 'outer,
            }
        }
        if wakeup_count > 0 {
            continue;
        }
        // Other stages
    }
}
```

Here we use a counter to record the number of woke up futures. If there is any,
process those future first. The receive is non-blocking, so two possible error
will need to be handle:
1. `TryRecvError::Empty`: The channel is empty, stop trying to receive.
2. `TryRecvError::Disconnected`: All the producer are disconnected, which means all futures are done, exit main loop directly.

## Stage 3 and Stage 4
```rust
'outer: loop {
    if let Some(index) = self.queue.pop_back() {
        // Stage 1
    } else {
        // Stage 2
        match self.rx.try_recv() {
            Ok(Message::Run(index)) => {
                self.queue.push_front(index);
            }
            Err(TryRecvError::Empty) => {
                if let Err(e) = reactor.wait(Some(Duration::from_millis(50))) {
                    log::error!("reactor wait error: {}, exit", e);
                    break;
                }
            }
            Ok(Message::Close) | Err(TryRecvError::Disconnected) => break,
        }
    }
}
```

These are the last stage in the main loop. Here the runtime will wait for any schedule messages
send by the global spawner, whether to run a new future or shutdown the runtime.
If there are no messages from the global spawner, we start to wait for reactor to wake up some futures.

Note that we need to check messages from global spawner at the same time, so we can't use indefinite
waiting on reactor. A time period need to be chosen to block the loop for a decent small amount of time
without spuriously wake up the main thread too frequently.
