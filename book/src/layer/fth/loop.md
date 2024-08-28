# Poll loop

The poll loop is the main loop that the `Executor` runs, as discussed in that section:

```rust
// Executor::run()
fn run(mut self, reactor: &mut Reactor) -> io::Result<()> {
    // 'env
    thread::scope(|s| -> io::Result<()> {
        // 'scope
        log::debug!("Spawn threads under scope...");
        self.scheduler.setup_workers(s);
        log::info!("Runtime booted up, start execution...");
        loop {
            reactor.check_extra_wakeups();
            match reactor.wait(Some(Duration::from_millis(100)), || self.message_handler()) {
                Ok(false) => {}
                Ok(true) => break,
                Err(e) => match e.kind() {
                    io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock => {}
                    _ => {
                        log::error!("Reactor wait error: {}, shutting down...", e);
                        break;
                    }
                },
            }
        }
        log::info!("Execution completed, shutting down...");
        // shutdown worker threads
        self.scheduler.shutdown();
        log::info!("Runtime shutdown complete.");
        Ok(())
    })?;
    Ok(())
}
```

Here scoped thread is used to encapsulate the lifetime of `Scheduler`'s worker threads.

In each iteration, the `Reactor` will first check if any unused events are now required,
as `mio` defaults to edge triggered mode. Then, it will run `wait` to wait for any
event or schedule message income for 100 ms.
The loop will keep running until a shutdown message or an IO error other than
`EINTR` or `EWOULDBLOCK` occurs.
