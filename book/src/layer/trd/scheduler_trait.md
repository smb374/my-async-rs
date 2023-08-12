# Scheduler trait design

To begin with, we'll list the key functions for a scheduler:

1. Be able to interpret commands that is sent by `Executor`.
2. Spawn and handle worker threads.
3. Schedule tasks depending on the scheduling strategy.

Since 3 is implementation dependent, our trait design will include the following
functions to fulfill the requirements in 1 and 2:

- `init()`: The `init()` function should take an argument `size` that specifies the number of threads.
  - It will spawn the worker threads and set up a channel for message passing.
  - All internal channels and structures should be initialized in this function.
  - It will return an instance of itself and the `Spawner`, as mentioned before.
- `schedule()`, `reschedule()`, `shutdown()`: The functions corresponding to `ScheduleMessage`.
  - Note that `shutdown()` should consume itself, so it will take `self` instead of `&self` or `&mut self`.
- `receiver()`: Return the receiver half of the message passing.
  - The sender half is held by `Spawner` that is returned by `init()`.

## Code

With the requirement, we can define our trait like the following code:

```rust
pub trait Scheduler {
    fn init(size: usize) -> (Spawner, Self);
    fn schedule(&mut self, index: FutureIndex);
    fn reschedule(&mut self, index: FutureIndex);
    fn shutdown(self);
    fn receiver(&self) -> &Receiver<ScheduleMessage>;
}
```
