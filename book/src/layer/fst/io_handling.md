# General IO handling

Generally, when using asynchronous runtime in Rust, you're dealing with non-blocking IO,
you'll want to make the file descriptors you're working on.

Setting a file descriptor is easy by using `fcntl` to set the `O_NONBLOCK`.

Next we'll talk about the error handling part.

A non-blocking IO handling can be described by the following pseudocode:

```
run_nbio(io):
    n <- io()
    if n == -1 then
        if (errno == EWOULDBLOCK) then
            // The io action will block, try again later
            return PENDING, nil
        else if (errno == EINTERRUPT) then
            // The action is interrupted by system, retry immediately
            return nbio()
        else
            // io is completed with error
            // other errors are handled by the user
            return READY, -1
        end
    else
        // io is completed
        return READY, n
    end
```

Basically the code will only handle two kinds of error itself: `EWOULDBLOCK` and `EINTERRUPT`:

1. `EWOULDBLOCK` means that `io()` will block the thread, return `PENDING` to make the user try again later.
2. `EINTERRUPT` means that an interrupt occurs when running `io()`, retry immediately.

We can then bring this to Rust:

```rust
// in IoWrapper::poll_ref()
fn poll_ref<U, F>(
    &self,
    cx: &mut Context<'_>, // context for polling
    interest: Interest, // READ or WRITE
    mut f: F, // action
) -> Poll<io::Result<U>>
where
    F: FnMut(&Self) -> io::Result<U>,
{
    match f(self) {
        // WouldBlock, do it when the io is available.
        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
            let current = self.token.load(Ordering::Relaxed);
            // Register to the reactor.
            // The reactor will wake the task to run this again
            // when it gets the event correspoding to the interst.
            self.register_reactor(current, interest, cx)?;
            Poll::Pending
        }
        // Interrupt, retry immediately
        Err(ref e) if e.kind() == io::ErrorKind::Interrupted => self.poll_ref(cx, interest, f),
        // Ready with Success or other Error
        r => Poll::Ready(r),
    }
}
```

Next, we'll talk about `IoWrapper`'s design.
