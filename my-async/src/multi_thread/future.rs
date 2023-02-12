use core::{
    hash::Hash,
    task::{Context, Poll, Waker},
};
use std::io;

use flume::Sender;
use futures_lite::future::Boxed;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use sharded_slab::{Clear, Pool};
use waker_fn::waker_fn;

// global future allocation pool.
pub static FUTURE_POOL: Lazy<Pool<BoxedFuture>> = Lazy::new(Pool::new);

/// Index for accessing future task.
///
/// By design, all future tasks will be store in a global [`Pool`][sharded_slab::Pool]
/// to reuse any allocation of previous future tasks.
///
/// The pool will return a unique index to access the future task in pool.
///
/// Since the future access is index-based, other interthread communication method will
/// be much trivial and easier to use/implement.
///
/// It also contains other counters for priority use in [`HybridScheduler`][1] and [`BudgetFuture`][2].
///
/// [1]: crate::schedulers::hybrid::HybridScheduler
/// [2]: crate::BudgetFuture
#[derive(Clone, Copy, Eq)]
pub struct FutureIndex {
    pub key: usize,
    pub(crate) budget_index: usize,
    pub(crate) sleep_count: usize,
}

impl PartialEq for FutureIndex {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl Hash for FutureIndex {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.key.hash(state);
    }
}

pub struct BoxedFuture {
    pub future: Mutex<Option<Boxed<io::Result<()>>>>,
    pub join_waker: Mutex<Option<Waker>>,
}

impl Default for BoxedFuture {
    fn default() -> Self {
        BoxedFuture {
            future: Mutex::new(None),
            join_waker: Mutex::new(None),
        }
    }
}

impl Clear for BoxedFuture {
    fn clear(&mut self) {
        self.future.get_mut().clear();
    }
}

impl BoxedFuture {
    pub fn run(&self, index: &FutureIndex, tx: Sender<FutureIndex>) -> bool {
        let mut guard = self.future.lock();
        // run *ONCE*
        if let Some(fut) = guard.as_mut() {
            let new_index = FutureIndex {
                key: index.key,
                budget_index: index.budget_index,
                sleep_count: index.sleep_count + 1,
            };
            let waker = waker_fn(move || {
                tx.send(new_index).expect("Too many message queued!");
            });
            let cx = &mut Context::from_waker(&waker);
            match fut.as_mut().poll(cx) {
                Poll::Ready(r) => {
                    if let Err(e) = r {
                        log::error!("Error occurred when executing future: {}", e);
                    }
                    true
                }
                Poll::Pending => false,
            }
        } else {
            true
        }
    }
}
