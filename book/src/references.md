# References

## Main references

Below is a list of articles and code that inspired me during the process of constructing
`my-async`:
- Asynchronous Programming in Rust: [https://rust-lang.github.io/async-book/](https://rust-lang.github.io/async-book/)
- `tokio`:
  - Their blog posts: [https://tokio.rs/blog](https://tokio.rs/blog)
  - Code: [https://github.com/tokio-rs/tokio](https://github.com/tokio-rs/tokio)
- Blog post: [https://levelup.gitconnected.com/explained-how-does-async-work-in-rust-c406f411b2e2](https://levelup.gitconnected.com/explained-how-does-async-work-in-rust-c406f411b2e2)
  - This blog post has some nice explanation and graphs that gives me the general idea of an async runtime
    and helps me quickly construct my initial runtime structure.

## Related works and Further reading
Below is a list that I found worth reading when doing this project:
- The Node Experiment - Exploring Async Basics with Rust: [https://cfsamson.github.io/book-exploring-async-basics/](https://cfsamson.github.io/book-exploring-async-basics/)
  - A piece of work that I found recently when finishing this document. It gives a great introduction to many
    knowledge that you might need to know (like the OS part that this document didn't cover).
- `monoio` by bytedance, inc.: [https://github.com/bytedance/monoio](https://github.com/bytedance/monoio)
  - Blog post about it, in a series of 5 posts: [https://www.ihcblog.com/rust-runtime-design-1/](https://www.ihcblog.com/rust-runtime-design-1/) (Note that it's in Chinese)
  - A work that utilizes the GATs feature and uses `io-uring` as the underlying reactor. I find their
    blog posts also explained a lot of details. The blog posts are in Simplified Chinese, though.
    Prepare a translator if you can't read Chinese.

- Code of `tokio`, `async-std`, and `smol`: You probably can have an easier time when reading the code
  of these bigger projects compared to mine. The ultimate goal of `my-async` and this document
  is to help you to understand how an async IO runtime can be implemented and build a picture
  when reading these code.
