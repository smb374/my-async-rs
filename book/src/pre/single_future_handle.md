# Future handling

Since the compiler will generate any `async fn` functions as `Future` objects,
the generic type will be a hurdle to program, allocating these `Future` objects
as boxed trait objects is more sensable and easier to program.

First we define a `BoxedLocal<T>` type alias and `BoxedFuture` type:
```rust ,noplaypen
type BoxedLocal<T> = Pin<Box<dyn Future<Output = T> + 'static>>;

struct BoxedFuture {
    future: RefCell<Option<BoxedLocal<io::Result<()>>>>,
}
```

Here we use `RefCell` for internal mutability and we assumed that all
enclosed should return `std::io::Result<()>` as `Future` requires to mutate it self
by its trait definition and most of the tasks are IO bounded to be used under async environments.

To handle the underlying `async fn`, we define the `run` function as the following:
```rust
impl BoxedFuture {
    fn run(&self, index: &FutureIndex, tx: Sender<FutureIndex>) -> bool {
        let mut guard = self.future.borrow_mut();
        // run *ONCE*
        if let Some(fut) = guard.as_mut() {
            let new_index = FutureIndex {
                key: index.key,
            };
            // Create a waker that sends back the future to the process queue
            // once it's woke by that reactor.
            let waker = waker_fn(move || {
                tx.send(new_index).expect("Too many message queued!");
            });
            // Create a Context from the waker we just created.
            let cx = &mut Context::from_waker(&waker);
            // Poll the underlying future with cx
            match fut.as_mut().poll(cx) {
                Poll::Ready(r) => {
                    if let Err(e) = r {
                        // log error to logging facility
                        log::error!("Error occurred when executing future: {}", e);
                    }
                    true // Return value ready
                }
                Poll::Pending => false, // Pending
            }
        } else {
            true // Finished already
        }
    }
}
```

The process goes by:
1. Check if it's done already for handling spurious call.
2. Create a waker that sends the index of itself back to process queue.
3. Create a `Context` for future polling using the waker we've just created.
4. Poll the future and match the result.

Next, we'll talk about the global storage.
