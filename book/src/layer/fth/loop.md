# Poll loop
After all the stuff defined, there's one more problem remain: Where to call `Reactor::wait()`?
1. Use a isolated thread specifically for the `Reactor` to do its stuff
2. Use a `Mutex` and make it a global to let workers call it.

I choose the first one since `Poll::wait()` requires `&mut` unique reference to call, and `check_extra_wakeups`
will become a long critical section if there is loads of events that arrives before someone actually need it.
The reason that not using main thread to do this is because that the main thread will have `Executor`'s message
handling loop running.

In `Executor` initialization, we set up a `poll_thread`:
```rust
// Executor::new()...
let poll_thread_handle = thread::Builder::new()
    .name("poll_thread".to_string())
    .spawn(move || Self::poll_thread())
    .expect("Failed to spawn poll_thread.");
// ...
fn poll_thread() {
    let mut reactor = reactor::Reactor::default();
    reactor.setup_registry();
    loop {
        // check if wakeups that is not used immediately is needed now.
        reactor.check_extra_wakeups();
        match reactor.wait(Some(Duration::from_millis(100))) {
            Ok(true) => break,
            Ok(false) => continue,
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => {
                log::error!("reactor wait error: {}, exit poll thread", e);
                break;
            }
        }
    }
}
```
We use a waiting time of 100ms to prevent the all-blocking scenario: Every thread is waiting on something and
not able to process new events.
This happens when I uses `reactor::wait(None)` for the `Reactor` to block until a readiness event comes in.
Because of this, I changed it to 100ms waiting duration to make it not so active but keep running.
I'me still researching on how to deal with this, but for the current state, I works like a charm.
