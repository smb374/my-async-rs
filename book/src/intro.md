# Introduction

This book is about the design and the implementation of `my-async`, a Rust async IO runtime.
Why creating a new runtime instead of using `tokio`? The problem of `tokio` is that its codebase is
huge and hard to trace down, a least for me.

When I was trying to understand `tokio`'s approach to async,
there are more than 35000+ lines of code, it's just too much for a CS student like me.
So I decided to work on a new runtime with a clear documentation
over the design and implementation as my graduate project.

`my-async` has the following goals:
- A convenient interface to wrap over `AsFd` types.
- A `Scheduler` trait that makes applying new scheduling strategy easy.
- A relatively short code that is easy to read.
- Having a decent performance compare to `tokio`.
- Clear documentation that can express its design and implementation.

Without a doubt, let's get started.
