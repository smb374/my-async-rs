# IO event registeration

## `Reactor` struct definition
Since our goal is to build an IO runtime, naturally we'll need to register IO
events we want to watch so that we can use a event based approach.

To achieve this, I use a library called `mio`, which is a library for non-blocking
system IO that wraps epoll, IOCP, kqueue, etc.

The library provides:
- `Poll` for polling system IO events.
    - Note that this is different from the `Poll` returned by `Future::poll()`
- `Events` specificaslly for `Poll::wait()` to store returned events
- `Registry` that accept using reference to register events.
- `Token` that wraps a `usize` as event's token.

We can then create a `Reactor` struct around `mio`'s structs:
```rust
static REGISTRY: OnceCell<Registry> = OnceCell::new();
static POLL_WAKE_TOKEN: Token = Token(usize::MAX);
pub(super) static POLL_WAKER: OnceCell<mio::Waker> = OnceCell::new();

pub struct Reactor {
    poll: Poll,
    events: Events,
    extra_wakeups: Vec<usize>,
}
```

### Note
`REGISTRY` is a global variable for global event register, since register only requires reference,
we can simply guard it with a one-time initialization atomic cell.

`POLL_WAKER` and `POLL_WAKE_TOKEN` is used for shutdown signal. The waker is created by `mio::Waker::new()`.

The `extra_wakeups` is used to store event tokens that arrived before anyone await it.
This is required as `mio` uses edge-triggered mode for the underlying system IO mechanism, without
this you'll miss events under high loading scenarios.

## Event handling
First of all, the code:
```rust
static WAKER_SLAB: Lazy<Slab<Mutex<Option<Waker>>>> = Lazy::new(Slab::new);

impl Reactor {
    pub fn wait(&mut self, timeout: Option<Duration>) -> io::Result<bool> {
        self.poll.poll(&mut self.events, timeout)?;
        if !self.events.is_empty() {
            log::debug!("Start process events.");
            for e in self.events.iter() {
                if e.token() == POLL_WAKE_TOKEN {
                    return Ok(true);
                }
                let idx = e.token().0;
                let waker_processed = process_waker(idx, |guard| {
                    if let Some(w) = guard.take() {
                        w.wake_by_ref();
                    }
                });

                if !waker_processed {
                    self.extra_wakeups.push(idx);
                }
            }
        }
        Ok(false)
    }
    pub fn check_extra_wakeups(&mut self) -> bool {
        let mut event_checked = false;
        self.extra_wakeups.retain(|&idx| {
            let waker_processed = process_waker(idx, |guard| {
                if let Some(w) = guard.take() {
                    event_checked = true;
                    w.wake_by_ref();
                }
            });
            !waker_processed
        });
        event_checked
    }
}

fn process_waker<F>(idx: usize, f: F) -> bool
where
    F: FnOnce(&mut MutexGuard<Option<Waker>>),
{
    if is_registered(idx) {
        if let Some(mutex) = WAKER_SLAB.get(idx) {
            let mut guard = mutex.lock();
            f(&mut guard);
            drop(guard);
            true
        } else {
            false
        }
    } else {
        false
    }
}

pub(crate) fn is_registered(token: usize) -> bool {
    WAKER_SLAB.contains(token)
}

pub(crate) fn add_waker(token: usize, waker: Waker) -> Option<usize> {
    let waker_found = process_waker(token, |guard| {
        if let Some(w) = guard.replace(waker.clone()) {
            w.wake_by_ref();
        }
    });
    if waker_found {
        None
    } else {
        WAKER_SLAB.insert(Mutex::new(Some(waker)))
    }
}

pub(crate) fn remove_waker(token: Token) -> bool {
    WAKER_SLAB.remove(token.0)
}

pub fn register<S>(
    source: &mut S,
    token: Token,
    interests: Interest,
    reregister: bool,
) -> io::Result<()>
where
    S: Source + ?Sized,
{
    if let Some(registry) = REGISTRY.get() {
        if reregister {
            registry.reregister(source, token, interests)?;
        } else {
            registry.register(source, token, interests)?;
        }
    } else {
        log::error!("Registry hasn't initialized.")
    }
    Ok(())
}
```
- `WAKER_SLAB` provides a global waker slab that will return a index to access the waker after insertion.
    - The returned index will be used as the event token that is registered with `REGISTERY`.
    - Since the same index may link to different wakers at different times, the entry is wrapped with `Mutex` to be able to replace contained `Waker`.
- `process_waker` will check whether a `Waker` is present when a event occurrs. If `WAKER_SLAB` doesnt' contained it or it's `None` at current state,mark return `false` to indicate that this is an extra wakeup that need to be handled after.
- `add_waker` will check whether a `Waker` exists in `WAKER_SLAB` with the token provide. If exists, swap the old one out and wake the old one, and return `None`. Otherwise, insert the waker and return a valid token for the caller to update.
    - By default `IoWrapper` will use `usize::MAX` as token. This will insert the waker and update its token to a valid one, then it can use `register()` to register the event it need with the updated token.
- `check_extra_wakeups` will linearly check whether a `Waker` is currently present in `WAKER_SLAB` with scanned index in `extra_wakeups`.
    - This function will be called with `wait()` in a set to check if any of the events in `extra_wakeups` is needed after each `wait()`.
