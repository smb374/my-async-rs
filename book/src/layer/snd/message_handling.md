# Design of Executor

By the previous section, we can now create a pseudocode to illustrate the command handling:

```
HandleRequests():
    while true:
        r <- GetRequest()
        match r:
            Spawn(Task) -> Put the Task to the execution system
            Shutdown -> break
```

The execution system is done by the `Scheduler`, and we also need to call `Reactor` to look up for
IO events to wake tasks. By these facts and the above pseudocode, the `Executor` can be viewed
as an abstraction layer that handles messages and relay the actual actions to `Scheduler` and `Reactor`.

The `Executor` also provides a function: `block_on`. The asynchronous function that's spawned by the `block_on` function
is just like the `main` function in a normal program: the runtime won't exit before this function ends without
further errors or interrupts occur during runtime.

In the current implementation, `Executor::new()` will set up a channel for scheduler message to pass, and
`Executor::block_on()` will act as an entry point to set up the `Reactor`, the global registry to register
IO events, a notifier to the `Reactor`, and the worker threads of the `Scheduler`.
The message handler is executed when the `Reactor` is notified by the notifier, which means there are at least one
new messages to be handled inside the message passing channel. For simplicity, the `Reactor` is notified
whenever a task is spawned in current implementation.

Next, we'll talk about the message passing in this design.
