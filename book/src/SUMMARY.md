# Summary
[Introduction](intro.md)

# Prerequisite Knowledges
- [Async in Rust](pre/async_in_rust.md)
- [Overview of an executor's architechture](pre/overview.md)
- [A minimal single-threded Future evaluator](pre/single_thread_executor.md)
    - [Future handling](pre/single_future_handle.md)
    - [Global Storages](pre/single_global_storage.md)
    - [Message Passing](pre/single_message_passing.md)
    - [Executor Main Loop](pre/single_executor.md)
    - [Final Code](pre/single_final_code.md)

# First layer - Future and IoWrapper
- [Bone of async - Future trait](layer/fst/future_trait.md)
    - [Future in depth](layer/fst/future_in_depth.md)
        - [Future trait mechanism](layer/fst/mechanism.md)
        - [Future desugar - a Finite State Machine](layer/fst/fsm.md)
        - [The challenge of managing Future objects](layer/fst/challenge.md)
    - [Generic Future handling](layer/fst/handling.md)
        - [General heap-allocated Future object](layer/fst/heap_alloc.md)
        - [Global Reusable Object Pool for allocation reuse, fragment control, and easy managment](layer/fst/pool.md)
- [IO Adapter for general file descriptor - IoWrapper](layer/fst/io_wrapper.md)
    - [General IO handling](layer/fst/io_handling.md)
    - [IoWrapper design](layer/fst/io_wrapper_design.md)

# Second layer - Executor and message passing
- [Heart of a runtime - Executor](layer/snd/executor.md)
    - [General commands of a runtime](layer/snd/commands.md)
    - [Design of Executor](layer/snd/message_handling.md)
- [Passing messages](layer/snd/message_passing.md)
    - [Spawner - a message sender](layer/snd/spawner.md)
    - [Message payload](layer/snd/message_payload.md)
- [Join Handle for Future](layer/snd/join_handle.md)

# Third layer - Scheduler and schedule problems
- [Multithread mania - Scheduler](layer/trd/scheduler.md)
    - [Scheduler trait design](layer/trd/scheduler_trait.md)
    - [General Worker structure and logic](layer/trd/worker_structure.md)
    - [The procedure of task scheduling](layer/trd/schedule_procedure.md)
- [Threading Method](layer/trd/threading_method.md)
    - [Round Robin](layer/trd/round_robin.md)
    - [Work Stealing](layer/trd/work_stealing.md)
    - [Hybrid Queue for Prioritized Work Stealing](layer/trd/hybrid.md)
- [A token bucket like algorithm for auto task yielding](layer/trd/token_bucket.md)

# Fourth layer - Reactor and Waker handling
- [System IO Event Harvester - Reactor](layer/fth/reactor.md)
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
