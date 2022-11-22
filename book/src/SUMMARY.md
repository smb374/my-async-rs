# Summary
[Introduction](intro.md)

# Prerequiste Knowledges
- [Async in Rust](pre/async_in_rust.md)
- [Overview of an executor's architechture](pre/overview.md)
- [A minimal single-threded `Future` evaluator](pre/single_thread_executor.md)

# First layer - Future and IoWrapper
- [Bone of async - `Future` trait](layer/fst/future_trait.md)
  - [`Future` in depth](layer/fst/future_in_depth.md)
    - [`Future` desugar - a Finite State Machine](layer/fst/fsm.md)
    - [`Future` trait mechanism](layer/fst/mechanism.md)
    - [The challenge of implementing an executor](layer/fst/challenge.md)
  - [Generic `Future` handling](layer/fst/handling.md)
    - [General heap-allocated `Future` object](layer/fst/heap_alloc.md)
    - [Global Reusable Object Pool for fragment controll and `Future` managment](layer/fst/pool.md)
- [IO Adapter for general file descriptor - `IoWrapper`](layer/fst/io_wrapper.md)
  - [General IO handling](layer/fst/io_handling.md)
  - [`IoWrapper` design](layer/fst/io_wrapper_design.md)

# Second layer - Executor and message passing
- [Heart of a runtime - `Executor`](layer/snd/executor.md)
  - [General commands of a runtime](layer/snd/commands.md)
  - [Design of `Executor` - an abstraction to scheduler and reactor](layer/snd/message_handling.md)
- [Passing messages](layer/snd/message_passing.md)
  - [`Spawner` - message sender](layer/snd/spawner.md)
  - [Message payload](layer/snd/message_payload.md)
- [Join Handle for `Future`](layer/snd/join_handle.md)

# Third layer - Scheduler and schedule problems
- [Multithread mania - `Scheduler`](layer/trd/scheduler.md)
  - [`Scheduler` trait design](layer/trd/scheduler_trait.md)
  - [General `Worker` structure and logic](layer/trd/worker_structure.md)
- [Threading Method](layer/trd/threading_method.md)
  - [Round Robin](layer/trd/round_robin.md)
  - [Work Stealing](layer/trd/work_stealing.md)
  - [Hybrid Queue for Prioritized Work Stealing](layer/trd/hybrid.md)
- [A token bucket like algorithm for auto task yielding](layer/trd/token_bucket.md)

# Fourth layer - Reactor and Waker handling
- [System IO Event Harvester - `Reactor`](layer/fth/reactor.md)
  - [IO event registeration](layer/fth/io_registration.md)
  - [Event handling](layer/fth/event_handling.md)
  - [Event maintain for late use](layer/fth/event_maintain.md)
- [Waker handling](layer/fth/waker_handling.md)
  - [Global slab for wakers](layer/fth/global_map.md)
  - [Waker registration](layer/fth/registration.md)

# Unresolved Problems and Future Works
- [Load Balancing](prob/load_balancing.md)
- [Generic Message Payload](prob/generic_payload.md)
- [Reactor abstraction for different systems](prob/reactor_abstract.md)
# References
- [References](references.md)
