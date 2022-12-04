# The challenge of managing Future objects

With the previous sections, we can know that:
1. `Future` object is lazy.
2. `Future` object needs a `Context`, which is obtained by a `Waker`, to be polled.
3. Generated `Future` object can contain numbers of anonymous states and fields.
4. Generated `Future` object is always `!Unpin`.

Managing the generated `Future` objects is quite a hurdle, as it's impossible to type
any `Future` statically so that all the generated `Future` objects runs on stack,
and it's quite inefficient to actively poll every `Future` object you get with a
empty `Waker` (basically busy waiting).

In the next section, I will discuss how to manage these generated bastards using heap.
While it's not zero-cost, managing the generated `Futures` as heap-allocated trait objects
is much simpler tahn fighting with the borrowchecker and the moving problems caused by `!Unpin`.
