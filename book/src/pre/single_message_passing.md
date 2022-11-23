# Message Passing
The `SPAWNER` we defined in the last part is used as a global message sender,
the underlying type is defined as:
```rust
struct Spawner {
    tx: Sender<Message>,
}

enum Message {
    Run(FutureIndex),
    Close,
}
```
while the receiver half is held by tthe executor.
The are two messages: `Run` and `Close`:
- `Run` is used to put the spawned future into the process queue.
- `Close` is used to signal the executor to shutdown.

With the messages defined, we need to implement the corresponding functions:
```rust
impl Spawner {
    fn spawn<F>(&self, future: F)
    where
        F: Future<Output = io::Result<()>> + 'static,
    {
        // Alloc future inside the pool and retrieve its key to the entry.
        let key = FUTURE_POOL.with(|p| {
            p.create_with(|seat| {
                seat.future.borrow_mut().replace(Box::pin(future));
            })
            .unwrap()
        });
        // Send run command to the executor.
        self.tx
            .send(Message::Run(FutureIndex {
                key,
            }))
            .expect("too many task queued");
    }
    fn shutdown(&self) {
        // Send shutdown command to the executor.
        self.tx.send(Message::Close).expect("too many task queued");
    }
}

// Corresponding public function for global spawner access.

pub fn spawn<F>(fut: F)
where
    F: Future<Output = io::Result<()>> + 'static,
{
    SPAWNER.with(|s| {
        if let Some(spawner) = s.borrow().as_ref() {
            spawner.spawn(fut);
        }
    })
}

pub fn shutdown() {
    SPAWNER.with(|s| {
        if let Some(spawner) = s.borrow().as_ref() {
            spawner.shutdown();
        }
    })
}
```

Note that the join handle is not implemented in this single-threaded executor for simplicity.
For the join handle implementation, please look [Join Handle for Future](../layer/snd/join_handle.md).

Finally, we come to the main loop that handles the message.
