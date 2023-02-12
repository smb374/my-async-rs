//! Multi-threaded executor.
//!
//! This multi-threaded executor needs a [`Scheduler`][b] for thread creation
//! and task scheduling.
//!
//! The executor itself on main thread won't do any work since it's main job is to receive
//! scheduling message from the spawner. There is also a IO polling thread for event notification.
//! The worker threads will be `(n - 2)` threads where `n` is the cpu number.
//!
//! For more information, see documentation for [`schedulers`][a].
//!
//! [a]: crate::schedulers
//! [b]: crate::schedulers::Scheduler

mod executor;
mod future;

pub use crate::schedulers::spawn;
pub use executor::Executor;
pub use future::FutureIndex;
pub(crate) use future::FUTURE_POOL;
