# Global Storages
By the design from me, every future should store in a global allocation pool for
allocation reuse and easy management. The details will be discussed in the
[Global Reusable Object Pool for fragment controll and Future managment](../layer/fst/pool.md)
part.

We first define `FUTURE_POOL` and `SPAWNER` as thread local objects as Rust assumes
you are under multi-threaded environment without specific instruction. This will
cause some bounds for global variables to guarentee memory safety.

```rust
thread_local! {
    static SPAWNER: RefCell<Option<Spawner>> = RefCell::new(None);
    static FUTURE_POOL: Pool<BoxedFuture> = Pool::new();
}
```

Since `Pool` is already lock-free, we don't need to use `RefCell` to encapsulate it.
The `SPAWNER` is used as an message sender and will be discuss in the next section.

We also need to define a `FutureIndex` that contains the key returned by the `Pool` and other
payloads(Though there are no other payloads in this case):
```rust
#[derive(Clone, Copy, Eq)]
struct FutureIndex {
    key: usize,
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

and implement `Clear` for the `BoxedFuture` for the allocation reserve part:
```rust
impl Clear for BoxedFuture {
    fn clear(&mut self) {
        self.future.borrow_mut().clear();
    }
}
```

Next, we'll move on to the message passing part.
