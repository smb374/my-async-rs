# Introduction

This book is about the design and the implementation of `my-async`, a Rust asynchronous IO runtime.
Why create a new runtime instead of using `tokio`? The problem with `tokio` is that its codebase is
vast and hard to track down, a least for me.

When I was trying to understand `tokio`'s approach to asynchronous runtime,
there were more than 35000 lines of code; it's just too much for a CS student like me.
So I decided to work on a new runtime with clear documentation
regarding the design and implementation as my graduate project.

`my-async` has the following goals:

- A convenient interface to wrap over `AsFd` types.
- A `Scheduler` trait that makes applying a new scheduling strategy easy.
- A relatively short code that is easy to read.
- Clear documentation that can express its design and implementation.

The code repo is here: [https://github.com/smb374/my-async-rs](https://github.com/smb374/my-async-rs).

## Note

This project still needs some polishes as the code is implemented
by one person. If you spot any problems, open an issue
at the repo. I will look at it and try to fix it if there is spare time.

There may be many places in this book that may seem awkward or
based on my misunderstanding to certain concepts, please don't hesitate to point them out.

Also, as a non-native English user, there are multiple grammar & spelling errors when writing this.
I will fix them as much as possible when I spot some,
but expect these errors when reading the book.

## Special thanks

I want to thank my supervisor for this project.
He helped me a lot with various concepts and spotted potential implementation flaws
during the development of this project.
Without him, the project may take longer to complete as I'm the only one
that works on the code of this project.
