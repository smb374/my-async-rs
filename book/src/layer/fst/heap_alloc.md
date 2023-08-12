# General heap-allocated Future object

Rust has the following ways to handle generic objects:

1. By `<T: Trait + 'lifetime>`: Specifies a type parameter with trait bound and lifetime bound.
2. By `impl Trait`: Use an anonymous type parameter bound by trait.
3. By `dyn Trait`: Use a trait object to dynamic dispatch type.

Method 1 needs to know the exact type in compile time, and method 2 can be only used at function signature,
our only option is by method 3, trait objects.

Since a trait object is an opaque type that the size is only known at runtime,
we need to use a pointer to access it.
As storing on stack needs to play with lots of lifetime bounds, w only consider these heap-allocated pointer types:

- `Pin<Box<dyn Future + AutoTraits + 'a>>`
- `Pin<Rc<dyn Future + AutoTraits + 'a>>`
- `Pin<Arc<dyn Future + AutoTraits + 'a>>`

A natural choice will be the `Box` one, since we only need to put it on heap.
You might think that doesn't `Future`'s `poll` function require `Pin<&mut Self>`? We don't need internal mutability inside?
As it turns out, we can make use of `AsMut` trait, that using `.as_mut()` will result in:

```
&mut Pin<P<T>> -> Pin<&mut T>
```

We only need the internal mutability stuff outside:

```rust
RefCell<Pin<Box<dyn Future + 'a>>> // Single thread.
Mutex<Pin<Box<dyn Future + 'a>>> // Multi thread.
```

These internal mutability needs a reference counted pointers to access properly, so we need to wrap it with `Rc` or `Arc`:

```rust
Rc<RefCell<Pin<Box<dyn Future + 'a>>>> // Single thread.
Arc<Mutex<Pin<Box<dyn Future + 'a>>>> // Multi thread.
```

or:

```rust
Pin<Rc<RefCell<dyn Future + 'a>>> // Single thread.
Pin<Arc<Mutex<dyn Future + 'a>>> // Multi thread.
```

At this point, you are good to go, although there may be a problem.
You see, you need to allocate every time a `Future` is spawned in your runtime. For a server type program
that spawns multiple handler that may be inefficient:

1. `Future` objects are allocated and destroyed multiple times, which can cause fragmentation depending on the global allocator.
2. The allocated space can actually be reused to reduce heap size and have a better performance as no allocation occurred when we reuse it.

That's why I change to use a global object pool provided by `sharded_slab`.
The next section I will talk about how to use a global object pool to handle `Future` objects.
