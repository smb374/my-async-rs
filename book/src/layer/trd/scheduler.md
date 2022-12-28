# Multithread mania - Scheduler
In the previous sections, we've look into `Executor`'s design
with some key component remain unexplained.
One of them is the `Scheduler` trait.

`Scheduler` is the part that is performing task scheduling and task execution.
In this part, we will discuss them in depth, including:
1. The `Scheduler` trait.
    - Trait design
    - General `Worker` structure
    - Scheduling procedure
2. Scheduling methods
    - Round Robin
    - Work Stealing
    - Hybrid Queue for Work Stealing
3. Auto task yielding algorithm

First, we will come to the trait itself...
