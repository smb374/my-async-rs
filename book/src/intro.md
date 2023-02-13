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
- Clear documentation that can express its design and implementation.

The repo of the code is located at here: [https://github.com/smb374/my-async-rs](https://github.com/smb374/my-async-rs).

## Note
This is a project that still needs some polishes as the code is implemented
all by myself. If you spot any problems, feel free to open an issue
at the repo. I will look at it and try to fix it if there are spare times.

## Special thanks
I want to thank my advisor during this project.
He helped me a lot on various concepts and spotting potential implementation flaws
during the development of this project.
Without him, the project may take longer to complete as I'm the only one
that works on the code of this project.
