# A minimal single-threded Future evaluator
This section will give an simple single-threaded executor implementation
for you to understand the whole picture of the following chapters, as
1. It doesnt't require a scheduler and any synchronisation.
2. The reacter is a relatively small factor to express the idea, we'll talk about it later.

If you want to see the reactor's part first, see [System IO Event Harvester - Reactor](../layer/fth/reactor.md)
and the following subsections.

Now, let's get started from `Future` handling.
