# Reactor abstraction for different systems
Currently the `Reactor` is implemented with the `mio` library,
which supports system IO on major systems. We can extend the `Reactor`
by turning it into a trait that accepts different reactor implementation
just like the `Scheduler` trait.

Possible extra implementations are:
- Completion based system IO, such as `io_uring`.
- Event through FFI or other schemes to bind existing systems.
  - One possible is that we can implement a reactor that can use results from JS promise under WASM environment.
  - Another one is that we can implement a reactor that can process XServer events through `xcb` that makes an async
    X11 runtime which can be used to make async X11 applications with Rust.

All of this will need the `Reactor` to be abstracted into a trait before we attempt to develop such
reactors.
