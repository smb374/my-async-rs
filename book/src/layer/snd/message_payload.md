# Message payload

The code in the previous section shows that the `FutureIndex` contains two extra payloads.
The two payloads are related to scheduling, that:

1. `sleep_count`: The sleep count is used in `HybridScheduler`, that it is used to prioritize the tasks. See [Hybrid Queue for Prioritized Work Stealing](../trd/hybrid.md)
2. `budget_index`: Index for retrieving budgets, that is used for auto task yielding. See [A token bucket like algorithm for auto task yielding](../trd/token_bucket.md)

Although the payloads are hard coded, when you design your own executor, you can store information that is used for
runtime metrics.

For generic payloads, you should make use of `Any` trait with `TypeId` to check whether the payload is the valid type.

Here is an example code:

```rust
struct FutureIndex {
    key: usize,
    // essential fields across schedulers
    // ...

    // generic payload that needs down casting.
    payload: Arc<Mutex<Option<Box<dyn Any + Send>>>>,
}

// Impl this for some other struct and set Target to your payload type.
trait ExtractPayload {
    type Target: 'static;
    fn extract(index: &FutureIndex) -> Option<Box<Self::Target>> {
        let p = index.payload.lock().unwrap().take();
        match p {
            Some(payload) => {
                if payload.as_ref().type_id() == TypeId::of::<Self::Target>() {
                    Some(payload.downcast().unwrap())
                } else {
                    None
                }
            }
            None => None,
        }
    }
}
```

Next, we'll move on to `JoinHandle`'s design.
