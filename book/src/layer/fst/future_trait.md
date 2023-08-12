# Future trait

`Future` trait is the core of asynchronous IO in Rust.
Without it, the `async`/`await` keyword won't work, and
designer of a runtime will have to implement a callback
system to cooperate with the system IO event handle
loop.

In this section, we will take a deeper look into
the `Future` trait and talk about the technique I
use to handle it.

Without a second thought, let's dig in...
