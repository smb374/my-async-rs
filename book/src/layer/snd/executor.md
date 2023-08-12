# Executor

In the previous section, we've described:

1. How to properly handle compiler generated `Future` objects
2. `IoWrapper` that is capable of being an IO source that provides IO events for waking tasks.

But defining these won't fire up the execution procedure, we still need an executor to process these
tasks. In this section, we'll discuss:

1. `Executor` for executing `Future`.
   - General commands of a runtime.
   - `Executor`'s design.
2. Passing messages.
   - `Spawner` - a message sender.
   - Message payload.
3. `JoinHandle` for spawned future.
