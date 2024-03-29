# Summary

[Introduction](intro.md)

# Prerequisite Knowledge

- [Asynchronous in Rust](pre/async_in_rust.md)
- [Overview of an executor's architecture](pre/overview.md)
- [A minimal single-threaded Future evaluator](pre/single_thread_executor.md)
  - [Future handling](pre/single_future_handle.md)
  - [Global Storage](pre/single_global_storage.md)
  - [Message Passing](pre/single_message_passing.md)
  - [Executor Main Loop](pre/single_executor.md)
  - [Final Code](pre/single_final_code.md)

# First layer - Future and IoWrapper

- [Future trait](layer/fst/future_trait.md)
  - [Future in depth](layer/fst/future_in_depth.md)
    - [Future trait mechanism](layer/fst/mechanism.md)
    - [Future internal - a Finite State Machine](layer/fst/fsm.md)
    - [The challenge of managing Future objects](layer/fst/challenge.md)
  - [Generic Future handling](layer/fst/handling.md)
    - [General heap-allocated Future object](layer/fst/heap_alloc.md)
    - [Global Reusable Object Pool for allocation reuse, fragment control, and easy management](layer/fst/pool.md)
- [IO Adapter for general file descriptor - IoWrapper](layer/fst/io_wrapper.md)
  - [General IO handling](layer/fst/io_handling.md)
  - [IoWrapper design](layer/fst/io_wrapper_design.md)

# Second layer - Executor and message passing

- [Executor](layer/snd/executor.md)
  - [General commands of a runtime](layer/snd/commands.md)
  - [Design of Executor](layer/snd/message_handling.md)
- [Passing messages](layer/snd/message_passing.md)
  - [Spawner - a message sender](layer/snd/spawner.md)
  - [Message payload](layer/snd/message_payload.md)
- [Join Handle for Future](layer/snd/join_handle.md)
<!-- - [UPDATE: Redesigned Executor logic](layer/snd/new_design.md) -->

# Third layer - Scheduler and schedule problems

- [Scheduler](layer/trd/scheduler.md)
  - [Trait design](layer/trd/scheduler_trait.md)
  - [General Worker structure and logic](layer/trd/worker_structure.md)
  - [The procedure of task scheduling](layer/trd/schedule_procedure.md)
- [Scheduling Method](layer/trd/scheduling_method.md)
  - [Round Robin](layer/trd/round_robin.md)
  - [Work Stealing](layer/trd/work_stealing.md)
  - [Hybrid Queue for Prioritized Work Stealing](layer/trd/hybrid.md)
- [A token bucket like algorithm for auto task yielding](layer/trd/token_bucket.md)

# Fourth layer - Reactor and Waker handling

- [System IO Event Harvester - Reactor](layer/fth/reactor.md)
  - [IO event registration](layer/fth/io_registration.md)
  - [Poll loop](layer/fth/loop.md)

# Unresolved Problems and Future Works

- [Load Balancing](prob/load_balancing.md)
- [Reactor abstraction for different systems](prob/reactor_abstract.md)

# References

- [References](references.md)
