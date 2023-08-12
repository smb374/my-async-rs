# Overview of an executor's architecture

The diagram of the executor's architecture:
![Executor Layer](../assets/Executor_Layer_Bright.png)

By the above diagram, we can divide the executor into four layers:

## First Layer - Future and IoWrapper
The job of this layer is to provide user interaction with the executor in a convenient way.
A global spawner is provided to send commands to the executor to spawn an async function or to shut down the executor.
The spawned function will return a join handle to join and retrieve the return value of that function.

`IoWrapper` is provided for convenient `AsFd` type wrapping. It also provides `ref_io` and `mut_io` for those
IO actions that are not predefined to run as async.

## Second layer - Executor
This layer is the surface of the executor. The main job of this layer is to handle the messages from the spawner and command
the underlying scheduler and reactor by the corresponding message.

The executor will also set up the global spawner, scheduler with all the worker threads, and the reactor thread
on init for the runtime to work. After the executor is initialized, the user must `block_on` a single main
async function to fire up the runtime.

## Third layer - Scheduler
This layer will adapt the defined scheduling strategy to schedule async tasks for the workers.
The provided `Scheduler` trait is an abstract layer that defines a scheduler's basic behavior for the executor.
This is crucial for the new scheduling strategy to plug into this runtime easily.

Currently, `RoundRobinScheduler`, `WorkStealingScheduler`, and `HybridScheduler` is implemented by default.

## Fourth layer - Reactor and Waker Handling
This layer will communicate with the system and harvest all IO events reported. Once the IO events are gathered,
the reactor will:

1. If wakers are registered related to an event, wake it up.
2. If not, store it in a table for later usage.

Since the underlying call is handled by `mio` that is edge-triggered, we need to check the stored events
with currently registered wakers to make sure every event is consumed.

The details of the layers will be discussed later in the following chapters, but first, I will give an implementation
of a single-threaded executor to help you understand the main idea of an executor, then we'll move on to the multi-threaded version.
