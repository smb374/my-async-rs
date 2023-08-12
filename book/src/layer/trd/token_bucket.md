# A token bucket like algorithm for auto task yielding

Reference: [Reducing tail latencies with automatic cooperative task yielding, `tokio`](https://tokio.rs/blog/2020-04-preemption)

The problem described in `tokio`'s blog is reasonable, and I think that it needs to be
countered one day, so here is my approach to the solution - `BudgetFuture` trait.

# The `BudgetFuture` trait

First, we'll introduce its interface:

```rust
pub trait BudgetFuture: FutureExt {
    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<Self::Output>
    where
        Self: Unpin,
    {
        poll_with_budget(self, cx)
    }
}

impl<F: FutureExt + ?Sized> BudgetFuture for F {}
```

The trait simply overloads the `poll` function with `poll_with_budget`, and its auto-implemented for
all types implementing `FutureExt`, which is auto-implemented for types implementing `Future`.

By this, if the user wants to enable this feature, the user can simply include this to use this feature.

Next, we'll look at the `poll_with_budget` function

## `poll_with_budget` function

The `poll_with_budget` function is defined with several helper functions, a slab, and some thread local storage global variables:

```rust
static BUDGET_SLAB: Lazy<Slab<AtomicUsize>> = Lazy::new(Slab::new);
thread_local! {
    pub(crate) static USING_BUDGET: Cell<bool> = Cell::new(false);
    static CURRENT_INDEX: Cell<usize> = Cell::new(usize::MAX);
    // cache to reduce atomic actions
    static BUDGET_CACHE: Cell<usize> = Cell::new(usize::MAX);
}

pub(crate) fn budget_update(index: &FutureIndex) -> Option<usize> {
    let mut result = None;
    let budget_index = index.budget_index;
    let current_budget = match BUDGET_SLAB.get(budget_index) {
        Some(b) => b.load(Ordering::Relaxed),
        None => {
            let idx = BUDGET_SLAB
                .insert(AtomicUsize::new(DEFAULT_BUDGET))
                .expect("Slab is full!!!");
            result.replace(idx);
            DEFAULT_BUDGET
        }
    };
    let old_index = CURRENT_INDEX.with(|idx| idx.replace(budget_index));
    let old_budget = BUDGET_CACHE.with(|b| b.replace(current_budget));
    if old_index != usize::MAX && old_budget != usize::MAX {
        if let Some(b) = BUDGET_SLAB.get(old_index) {
            b.store(old_budget, Ordering::Relaxed);
        }
    }
    result
}

pub(crate) fn poll_with_budget<T, U>(fut: &mut T, cx: &mut Context<'_>) -> Poll<U>
where
    T: FutureExt<Output = U> + ?Sized + Unpin,
{
    USING_BUDGET.with(|x| x.replace(true));
    BUDGET_CACHE.with(|budget| {
        let val = budget.get();
        // if budget is zero, reschedule it by immediately wake the waker then return Poll::Pending (yield_now)
        if val == 0 {
            cx.waker().wake_by_ref();
            budget.set(DEFAULT_BUDGET);
            return Poll::Pending;
        }
        match fut.poll(cx) {
            Poll::Ready(x) => {
                // budget decreases when ready
                budget.set(val - 1);
                Poll::Ready(x)
            }
            Poll::Pending => Poll::Pending,
        }
    })
}

pub(crate) fn process_future(mut index: FutureIndex, tx: &Sender<FutureIndex>) {
    if let Some(boxed) = FUTURE_POOL.get(index.key) {
        USING_BUDGET.with(|ub| {
            if ub.get() {
                if let Some(idx) = budget_update(&index) {
                    index.budget_index = idx;
                }
                ub.replace(false);
            }
        });
        let finished = boxed.run(&index, tx.clone());
        if finished {
            wake_join_handle(index.key);
            if !FUTURE_POOL.clear(index.key) {
                log::error!(
                    "Failed to remove completed future with index = {} from pool.",
                    index.key
                );
            }
        }
    } else {
        log::error!("Future with index = {} is not in pool.", index.key);
    }
}
```

Let's crack this down...

### Function body

First, the `USE_BUDGET` thread local variable will be replaced to `true` to indicate that we're using budget.
Then we use the budget value stored in `BUDGET_CACHE`, which is visible throughout the thread and set by `budget_update`,
to poll the future. If a task spends all of its budgets, it will return `Pending` immediately with its budget restored to the default value.
We also wake the future up immediately such that it will re-enter the processing queue at the same time we return `Pending`.

### `budget_update`

This function will be called every time a task is being processed by `process_future` if we're using budget.
The reason is that we'll cache a task's budget into thread local storage to lower the amount of atomic operation.
Whenever we switch to a new task, it will need to replace the cache with the current one in slab, and store the old one
back with atomic operation to update the information in slab.

The first time we call this function, the function will store a `AtomicUsize` inside the `BUDGET_SLAB`,
and return its index so the `budget_index` field will be a valid index.

### Variables

- `USE_BUDGET`: TLS variable, indicate whether we're using budget.
- `CURRENT_INDEX`: TLS variable, cache of the budget's index for current task. Used to index the atomic variable for storing the updated cache value back to slab.
- `BUDGET_CACHE`: TLS variable, cache of the budget's value for current task. Any budget operations will first write this variable, then write back to the slab when `budget_update`.
- `BUDGET_SLAB`: Global variable, slab storage of all budgets. It will give an index for first-time budget usage, which is used to index the budget in the slab.
