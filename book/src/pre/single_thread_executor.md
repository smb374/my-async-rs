# A minimal single-threaded Future evaluator
This section will give a simple single-threaded executor implementation
for you to understand the whole picture of the following chapters, as
1. It doesn't require a scheduler or any synchronization.
2. The reactor is a relatively minor factor in expressing the idea. We'll talk about it later.

If you want to see the reactor's part first, see [System IO Event Harvester - Reactor](../layer/fth/reactor.md)
and the following subsections.

Now, let's get started with `Future` handling.

