# IoWrapper design

`IoWrapper` is a wrapper for IO sources like `File`, `TcpStream`, etc. to become an IO event provider
for the runtime. The wrapper provides various defaults and methods for users to define a new IO source quickly.

The `IoWrapper` is defined as:

```rust
pub struct IoWrapper<T: AsFd> {
    inner: T,
    token: AtomicUsize,
}
```

Where `inner` is the type being wrapped, and `token` is the identifier.

The wrapped type is bounded by `AsFd` since the runtime only accepts registering IO events related
to system IO, the wrapper thus requires the types to be wrapped should be `AsFd` that can extract
the underlying file descriptor.

`IoWrapper` provides:

- A convenient wrapper over `AsFd` types that set to non-blocking mode automatically.
- `ref_io` and `mut_io` for IO that doesn't require/require mutating self.
  - `ref_io` and `mut_io` are also asynchronous function by themselves, you don't need to implement `Future` yourself.
  - You can use these function to perform IO operations that aren't defined by the runtime or other asynchronous traits.
  - For usage, see [API documentation](https://smb374.github.io/my-async-rs/api_references/my_async/struct.IoWrapper.html)
- Auto implementation of `AsyncRead` and `AsyncWrite` for types implementing `std::io::Read` and `std::io::Write`.
- Get reference/mutable reference of the inner type by `IoWrapper::inner()`/`IoWrapper::inner_mut()`.

In addition to these features, since a `IoWrapper` wraps over any `AsFd` type with a token, we can register it with the
token to the reactor of the runtime. The reactor will then look after its events, and thus can wake up tasks related to
it corresponding to `Interest::READABLE` or `Interest::WRITABLE`. In this way, `IoWrapper` can play the role of IO
events provider to wake asynchronous tasks. The rest of the details will be discussed in the
subsections of [System IO Event Harvester - Reactor](../fth/reactor.md).
