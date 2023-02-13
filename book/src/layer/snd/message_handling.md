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
IO events to wake tasks. By these facts and the above pseudo code, the `Executor` can be viewed
as an abstraction layer that handles messages and relay the actual actions to `Scheduler` and `Reactor`.

In the code, the `Executor` will do almost the same thing with one more message to handle. The `Executor`
will also spawn a thread for `Reactor` to run, as the IO events will need to be checked separately if
we have the message handling loop in the main thread.

The `Executor` also provides a function: `block_on`. The async function that's spawned by the `block_on` function
is just like the `main` function in a normal program: the runtime won't exit before this function ends without
further errors or interrupts occur during runtime.

The complete pseudocode for the `Executor`:
```
Init():
    Initialize the Scheduler.
    Spawn a thread to init and run the Reactor.
    Set error hook for graceful shutdown to clean storages.
    Return instance of self

HandleRequests():
    while true:
        r <- GetRequest()
        match r:
            Spawn(Task) -> Put the Task to the execution system
            Shutdown -> break

Run():
    HandleRequests()
    Send Shutdown message to the Reactor
    Join the thread for the Reactor

block_on(f):
    handle <- spawn(f())
    Run()
    Get the result from handle
    Return result of f()
```

Next, we'll talk about the message passing in this design.
