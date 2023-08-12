# Async in Rust

Async in Rust is simple and complex at the same time. If you're the user, you can do
the following stuff as an async 101:

```rust
#[tokio::main]
async fn main() -> io::Result<()> {
    println!("Hello async!");
    let result = async_io_fn_one().await?;
    println("Get IO result: {}", result);
    Ok(())
}
```

Just that simple.

On the other hand, if you want to dig inside, things can get quite tricky.

Async in Rust differs from other languages: it doesn't have a standard runtime.
The core language team decides to:

1. Maintain flexibility over implementations that is suitable to different scenarios
2. Keep the size of the standard library small.

The result of the decision is the `Future` trait. Anything that implements `Future` can
use the `.await` keyword to execute it asynchronously. The compiler can also implement
`Future` for normal function by adding `async` before the `fn` keyword.

The approach, however, causes a problem: by the design of the `Future` trait, it is lazy.
`Future` won't progress to the underlying code until someone uses the `poll()` method
to poll it. In other words, `Future` needs to be polled by some mechanism to work.
There are multiple ways to poll a `Future` object, most commonly by an executor.

The executor must be capable of dispatching, managing, and executing `Future`. The implementation of an executor can look very different, depending on the context, resources, and platform.
Generally, an executor can be divided into a few parts, that these parts will later discuss in the following sections.

Various executor implementations exist, such as `tokio`, `async-std`, `smol`, etc.
The problem is that they are incompatible since the executor must store numerous states to help
execute the `Future` objects. The problem is still unsolved now and hopefully can be solved one day.
