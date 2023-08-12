# Spawner - message sender

The sender half is defined as `Spawner`, which contains the sender half of the message passing channel.
When the `Executor` initialize, it will also initialize a global `Spawner` for user to use `spawn()` and `shutdown()`.

When calling `spawn()` or `Executor::block_on()`, the `Spawner` will do the following stuff before
`ScheduleMessage::Schedule` is sent:

1. Create a shared memory for the future handle to use
2. Add extra code after the future need to spawned that
   - Put the execution result to the shared memory.
   - Check if it is called by `block_on`, if yes, shutdown the runtime.
3. Allocate the `Future` in global `Future` object pool and get its key.
4. Construct the `FuturIndex` with the key and other payloads.
5. Send `ScheduleMessage::Schedule` with the `FutureIndex`.
6. Return the handle to the spawned future.

The code:

```rust
// Spawner::spawn_with_handle()
pub fn spawn_with_handle<F>(&self, future: F, is_block: bool) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    // Step 1.
    let result_arc: Arc<Mutex<Option<F::Output>>> = Arc::new(Mutex::new(None));
    // Step 2.
    let clone = result_arc.clone();
    let spawn_fut = async move {
        let output = future.await;
        // Store the result.
        let mut guard = clone.lock();
        guard.replace(output);
        // Check if the function is called by `Executor::block_on()`
        if is_block {
            log::info!("Shutting down...");
            shutdown();
        }
        Ok(())
    };
    // Step 3.
    let key = FUTURE_POOL
        .create_with(|seat| {
            seat.future.get_mut().replace(spawn_fut.boxed());
        })
        .unwrap();
    // Used in auto task yielding if enabled.
    let budget_index = BUDGET_SLAB
        .insert(AtomicUsize::new(DEFAULT_BUDGET))
        .unwrap();
    // Step 4., Step 5.
    self.tx
        .send(ScheduleMessage::Schedule(FutureIndex {
            key,
            budget_index,
            sleep_count: 0,
        }))
        .expect("Failed to send message");
    // Step 6.
    JoinHandle {
        spawn_id: key,
        registered: AtomicBool::new(false),
        inner: result_arc,
    }
}
```

For, `Shutdown` and `Reschedule`, the implementation is simple: simply send the corresponding message with necessary arguments.
We can also define `spawn()` and `shutdown()` now as their just a wrapper to call the global spawner and use its method call
to send messages to the `Executor`.
