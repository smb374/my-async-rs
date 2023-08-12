# System IO Event Harvester - Reactor

After all the stuff we talk about, we'll now talk about the `Reactor`, the main source for event.
Since you only need an event source to craft a reactor, it is possible that you can design a
non-IO asynchronous runtime in Rust. However, `my-async`'s main focus is building a IO runtime,
our reactor will stick to IO events.

In this section, we'll talk about:

- `Reactor` design
- `Waker` management

Let's get started...
