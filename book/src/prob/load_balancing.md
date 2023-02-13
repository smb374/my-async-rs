# Load Balancing
Load balancing is always a problem when it comes to multiple workers.
An efficient algorithm should:
1. Distribute tasks evenly that the final elapsed time is reduced
as there are no long tails in terms of worker execution time.
2. Having a relatively small overhead that the algorithm won't
take a large portion when analyzing the final elapsed time.

Currently the `RoundRobinScheduler` uses Round-Robin, and `WorkStealingScheduler` and `HybridScheduler`
will let the worker who comes first to get the task from the global task queue as they uses
work-stealing.

Further experiment needs to be conducted to find possibly better solution when it comes
to distributing tasks from the global task queue. If one is found, the runtime may generate
a better performance.

One possible direction is to estimate the execution time an spawned task by some
metrics and a heuristic function that can hopefully guesses the right worker to fill in.
This can take quite amount of time to assess and implement, and possibly have a worse performance
compare to current solutions.
