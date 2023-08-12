# Future internal - a Finite State Machine

In Rust, we can create an asynchronous function as simple as the following code:

```rust
async fn async_func() {
    init_step();
    f().await;
    g().await;
    h().await;
    final_step();
}
```

From a user's perspective, we can quickly conclude that the function will run
`f()`, `g()`, `h()` in order asynchronously. But how this snippet is related
to the `Future` trait? It turns out that the compiler will translate the code snippet
into the following:

```rust
struct AsyncFunc {
    fut_f: FutF,
    fut_g: FutG,
    fut_h: FutH,
    state: AsyncFuncState,
}

enum AsyncFuncState {
    Init,
    AwaitF,
    AwaitG,
    AwaitH,
    Final,
}

impl Future for AsyncFunc {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.get_mut();
        loop {
            match me.state {
                AsyncFuncState::Init => {
                    init_step();
                    me.state = AsyncFuncState::AwaitF;
                },
                AsyncFuncState::AwaitF => match me.fut_f.poll(cx) {
                    Poll::Ready(()) => me.state = AsyncFuncState::AwaitG,
                    Poll::Pending => break Poll::Pending,
                },
                AsyncFuncState::AwaitG => match me.fut_g.poll(cx) {
                    Poll::Ready(()) => me.state = AsyncFuncState::AwaitH,
                    Poll::Pending => break Poll::Pending,
                },
                AsyncFuncState::AwaitH => match me.fut_h.poll(cx) {
                    Poll::Ready(()) => me.state = AsyncFuncState::Final,
                    Poll::Pending => break Poll::Pending,
                },
                AsyncFuncState::Final => {
                    final_step();
                    break Poll::Ready(());
                },
            }
        }
    }
}
```

This code can be viewed as a finite state machine, and can be visualized as the following diagram:
![](https://i2.lensdump.com/i/RkBbkv.png)

Since the code is compiled to a finite state machine, the state can store the arguments for the future to run,
the return value of previous future execution, etc. Because of this, the use of `Pin` and `!Unpin` is necessary.

## Why use `Pin` and why the generated code implements `!Unpin`?

Consider the following `async` block:

```rust
async {
    let mut x = [0u8; 4096];
    let fut = read_to_buf(&mut x);
    let n = fut.await;
    println!("got {:?}", &x[..n]);
}
```

The compiler will generate the following structure:

```rust
struct ReadToBuf<'a> {
    buf: &'a mut [u8],
}
struct AsyncBlock1 {
    x: [u8; 4096],
    fut: ReadToBuf<'_>, // self refer to x field
    state: State,
}
```

We can see that the generated `AsyncBlock1` contains a future `ReadToBuf<'_>` that uses `&mut x` for reading.
If this `async` block is moved, so will `x`, and since `x` is moved, the pointer in `ReadToBuf<'_>` can point
to an invalid location or dangle, causing undefined behavior. To prevent this, the generated `Future` object
will always be `!Unpin` to enable the effect that `Pin` brings, and thus the `poll` function takes
`self: Pin<&mut Self>` rather than `&mut self`.
