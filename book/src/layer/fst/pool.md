# Global Reusable Object Pool for allocation reuse, fragment control, and easy managment

`sharded_slab` provides `Pool` for object pool that can:
1. Allocate inside the pool.
2. Retain the allocation inside the pool when an entry is cleared.
3. Reuse the retained allocation when allocation occurs.

The pool requires `T: Clear + Default`, that the `Clear` trait is used to clear value and retain allocation.
We can use this to design the type we want to store. In this case, the follwoing type is choosed:
```rust
// type Boxed<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;
pub(crate) struct BoxedFuture {
    pub(crate) future: Mutex<Option<Boxed<io::Result<()>>>>,
}
```

By default, `Option<T>` implements `Clear` by default, which uses `Option::take()` to do the clear job.
`Option::take()` itself is implmented by `std::mem::replace(self, None)`, which the `std::mem::replace`
won't dealloc the left hand side. This property makes it capable to do the allocation retain job, so we can
put the `Boxed` type under it. The `Mutex` wraps around it is to provide internal mutability for `Future`'s `poll` requirement.

We can implement `Clear` for `BoxedFuture` with `Option::clear()`:
```rust
impl Default for BoxedFuture {
    fn default() -> Self {
        BoxedFuture {
            future: Mutex::new(None),
        }
    }
}

impl Clear for BoxedFuture {
    fn clear(&mut self) {
        self.future.get_mut().clear();
    }
}
```
Then create a global future object pool with `Lazy` from `once_cell`:
```rust
// global future allocation pool.
pub(crate) static FUTURE_POOL: Lazy<Pool<BoxedFuture>> = Lazy::new(Pool::new);
```

The key will be wrpped inside `FutureIndex` that contains other payloads:
```rust
#[derive(Clone, Copy, Eq)]
pub struct FutureIndex {
    pub key: usize,
    // Other payloads...
}

impl PartialEq for FutureIndex {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl Hash for FutureIndex {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.key.hash(state);
    }
}
```

## The overall step of `Future` processing
1. Allocate future object inside the pool when a future is spawned. The pool will return a key to access the entry where the future is.
    ```rust
    // in Spawner::spawn_with_handle()
    // ...
    let key = FUTURE_POOL
        .create_with(|seat| {
            seat.future.get_mut().replace(spawn_fut.boxed());
        })
        .unwrap();
    // ...
    ```
2. The key is used instead of a pointer to be processed by the message system and the schedule system.
    ```rust
    // in Spawner::spawn_with_handle(), after step 1
    // ...
    self.tx
        .send(ScheduleMessage::Schedule(FutureIndex {
            key,
            // Other payloads...
        }))
        .expect("Failed to send message");
    // ...
    ```
3. When the future is going to be processed:
    1. The worker will use the key to gain the access of the future in pool.
        ```rust
        // in scheduler::process_future()
        if let Some(boxed) = FUTURE_POOL.get(index.key) {
            let finished = boxed.run(&index, tx.clone());
        } // ...
        ```
    2. Lock the future and gain `&mut` to use `Future::poll` and poll the future. Return if the future is `Ready`.
        ```rust
        // in BoxedFuture::run()
        let mut guard = self.future.lock();
        // run *ONCE*
        if let Some(fut) = guard.as_mut() {
            let waker = waker_fn(move || {
                // Create waker
            });
            let cx = &mut Context::from_waker(&waker);
            match fut.as_mut().poll(cx) {
                Poll::Ready(r) => {
                    if let Err(e) = r {
                        // log error
                    }
                    true
                }
                Poll::Pending => false,
            }
        } else {
            true
        }
        ```
4. If the future is `Poll::Ready`, clear the future for future allocation reuse.
    ```rust
    // in scheduler::process_future()
    if finished {
        // ...
        if !FUTURE_POOL.clear(index.key) {
            // ...
        }
    }
    ```

## Note
The `Future` object is now single accessed by a single thread, why use `Mutex` instead of `RefCell` as they both implement `Send`?
This is because that the pool itself needs `Sync` for the global pool encapsulate in `Lazy` to wrok properly in multi-threaded environment.
We need to use `Mutex` to make the type store inside the pool `Sync`.

The `Mutex` will have small impact since it's already single accessed, its function will be like a `RefCell` that the thread-blocking function is canceled by our design.
