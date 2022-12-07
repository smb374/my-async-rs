# Message payload

The code in the previous section shows that the `FutureIndex` contains two extra payloads.
The two payloads are related to scheduling, that:
1. `sleep_count`: The sleep count is used in `HybridScheduler`, that it is used to prioritize the tasks. See [Hybrid Queue for Prioritized Work Stealing](layer/trd/hybrid.md)
2. `budget_index`: Index for retrieving budgets, that is used for auto task yielding. See [A token bucket like algorithm for auto task yielding](layer/trd/token_bucket.md)

Although the payloads is hard coded, when you design your own executor, you can store informations that is used for
runtime metrics.

For generic payloads, you can use a trait object to achieve, such as:
```rust
trait GenericPayload {
    // methods and associated types in here
}

struct FutureIndex<'a> {
    key: usize,
    payload: Arc<Mutex<dyn GenericPayload + 'a>>, // for payload mutation
}
```

Anyways, that's all for this layer, we'll move on to the `Scheduler`.
