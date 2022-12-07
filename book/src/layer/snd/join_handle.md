# Join Handle for Future

As shown before, we use a shared memory to store the result of the spawned future, then we put the
shared memory into `JoinHandle` and return to the user.

The `JoinHandle` is defined as:
```rust
pub struct JoinHandle<T> {
    spawn_id: usize,
    registered: AtomicBool,
    inner: Arc<Mutex<Option<T>>>,
}
```
where
- `spawn_id` is the id of the spawned future.
- `registered` is a flag to check if the waker of it is registered.
- `inner` is the shared memory.

We can take a look at `join()` and `try_join()` with other structs and functions to see how it works:
```rust
static JOIN_HANDLE_MAP: Lazy<RwLock<FxHashMap<usize, Waker>>> =
    Lazy::new(|| RwLock::new(FxHashMap::default()));

pub struct FutureJoin<'a, T> {
    handle: &'a JoinHandle<T>,
}

impl<T> JoinHandle<T> {
    pub fn join(&self) -> FutureJoin<'_, T> {
        FutureJoin { handle: self }
    }
    pub fn try_join(&self) -> Option<T> {
        let mut guard = self.inner.lock();
        guard.take()
    }
    fn register_waker(&self, waker: Waker) {
        JOIN_HANDLE_MAP.write().insert(self.spawn_id, waker);
        self.registered.store(true, Ordering::Relaxed);
    }
    fn deregister_waker(&self) {
        JOIN_HANDLE_MAP.write().remove(&self.spawn_id);
        self.registered.store(false, Ordering::Relaxed);
    }
}

impl<'a, T> Future for FutureJoin<'a, T> {
    type Output = T;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.handle;
        // Lock the shared memory and try to take the content of it.
        let mut guard = me.inner.lock();
        match guard.take() {
            // If success
            Some(val) => {
                // Deregister the waker from JOIN_HANDLE_MAP
                if me.registered.load(Ordering::Relaxed) {
                    me.deregister_waker();
                }
                // Return Ready(val)
                Poll::Ready(val)
            }
            // Else
            None => {
                // Register waker if the waker is not registered
                if !me.registered.load(Ordering::Relaxed) {
                    // spawned future can use it's own id to wake JoinHandle.
                    me.register_waker(cx.waker().clone());
                }
                // Return Pending
                Poll::Pending
            }
        }
    }
}

// Called when a spawned future finishes running
// If the waker is registered, we wake it up
// Otherwise, the JoinHandle hasn't request join.
pub(super) fn wake_join_handle(index: usize) {
    if let Some(waker) = JOIN_HANDLE_MAP.read().get(&index) {
        waker.wake_by_ref();
    }
}
```

Since we use a shared memory that is `Arc<Mutex<Option<T>>>`, the `try_join()` can be implemented easily
by lock `inner` and take it. The `Option<T>`'s value can indicate the success or not.

`join()` is a async function that will wait until the future of the handle finishes execution, that is,
if `inner` is `None`, we register the waker of the context where you call `join()` to a map.
The entry will use the spawned future's key (`spawn_id`) as its key so that the future can use the waker to wake
the context where `join()` is being called.

Take the following code as an example:
```rust
async fn parent() {
    let handle = spawn(child());
    let result: T = handle.join().await; // waker is from parent's context
}

async fn child() -> T {
    // do stuff...
    result
}
```
`handle` will use the waker from context `cx` that is used to poll `parent`, while the waker registration uses
`child`'s index as its key so that `child` can wake `parent` to poll `handle.join()`.

Since we need to use the key of the spawned future as its key to the waker map, we can only use
a regular hashmap with a `RwLock` so that we can specify its key. This is the simplest way I've
tried so far, otherwise we need to make more complicated code so that we don't need to use a hashmap.

Anyways, that's all for the `Executor`, we'll now move one to the `Scheduler`.
