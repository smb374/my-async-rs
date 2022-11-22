# Async in Rust

Async in Rust is simple and complex at the same time. If you're the user, you can simply do
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

On the other hand, if you want to dig inside, thing can get quite tricky.

Async in Rust is kind of different from other languages: it doesn't have a standard runtime.
The decision is made by the core language team to:

1. Maintain fexibility over implementations that is suitable to different scenarios
2. Keep the size of the standard library small.

The result of the decision is the `Future` trait. Anything that implements `Future` can
use the `.await` keyword to execute it asynchronously. The compiler can also implement
`Future` for normal function by adding `async` before the `fn` keyword.

The approach however, cause a problem: by the design of the `Future` trait, it is lazy.
`Future` won't make any progress to the underlying code until someone use `poll()` method
to poll it. In other words, `Future` needs to be polled by some mechanism to work.
There are multiple ways to poll a `Future` object, most commonly by an executor.

The executor needs to be capable to dispatch, manage, and execute `Future`. The implementation
of an executor can look very different, depending on the context, resources, and platform.
Generally an executor can be divided to a few parts and will later be discussed in the next part.

There are numbers of executor implementations in exist, such as `tokio`, `async-std`, `smol`, etc.
The problem is that they are not compactible, since executor need to store numerous of states to help
executing the `Future` objects. The problem is still unsolved until now and hopefully can be solved one day.
