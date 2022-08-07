use std::{io, task::Waker, time::Duration};

use once_cell::sync::Lazy;
use parking_lot::{Mutex, MutexGuard};
use polling::{Event, Poller};
use rustix::fd::AsRawFd;
use sharded_slab::Slab;

static POLLER: Lazy<Poller> = Lazy::new(|| Poller::new().unwrap());
static WAKER_SLAB: Lazy<Slab<Mutex<Option<Waker>>>> = Lazy::new(Slab::new);

pub struct Reactor {
    events: Vec<Event>,
    extra_wakeups: Vec<usize>,
}

impl Reactor {
    pub fn new(capacity: usize) -> Self {
        let events = Vec::with_capacity(capacity);
        let extra_wakeups = Vec::with_capacity(capacity);
        Self {
            events,
            extra_wakeups,
        }
    }

    pub fn wait(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        self.events.clear();
        let n = POLLER.wait(&mut self.events, timeout)?;
        if !self.events.is_empty() {
            log::debug!("Start process events.");
            for e in self.events[..n].iter() {
                let idx = e.key;
                let waker_processed = process_waker(idx, |guard| {
                    if let Some(w) = guard.take() {
                        w.wake_by_ref();
                    }
                });

                if !waker_processed {
                    self.extra_wakeups.push(idx);
                }
            }
        }
        Ok(())
    }

    pub fn check_extra_wakeups(&mut self) -> bool {
        let mut event_checked = false;
        self.extra_wakeups.retain(|&idx| {
            let waker_processed = process_waker(idx, |guard| {
                if let Some(w) = guard.take() {
                    event_checked = true;
                    w.wake_by_ref();
                }
            });
            !waker_processed
        });
        event_checked
    }
}

impl Default for Reactor {
    fn default() -> Self {
        Self::new(1024)
    }
}

fn process_waker<F>(idx: usize, f: F) -> bool
where
    F: FnOnce(&mut MutexGuard<Option<Waker>>),
{
    if WAKER_SLAB.contains(idx) {
        let mutex = WAKER_SLAB.get(idx).unwrap();
        let mut guard = mutex.lock();
        f(&mut guard);
        drop(guard);
        true
    } else {
        false
    }
}

pub(crate) fn add_waker(key: usize, waker: Waker) -> Option<usize> {
    let waker_found = process_waker(key, |guard| {
        if let Some(w) = guard.replace(waker.clone()) {
            w.wake_by_ref();
        }
    });
    if waker_found {
        None
    } else {
        WAKER_SLAB.insert(Mutex::new(Some(waker)))
    }
}

pub(crate) fn remove_waker(key: usize) -> bool {
    WAKER_SLAB.remove(key)
}

pub fn register<T: AsRawFd>(source: &T, interest: Event, reregister: bool) -> io::Result<()> {
    if reregister {
        POLLER.modify(source, interest)?;
    } else {
        POLLER.add(source, interest)?;
    }
    Ok(())
}

pub(crate) fn deregister<T: AsRawFd>(source: &T, key: usize) -> io::Result<()> {
    remove_waker(key);
    // TODO: find a way to deregister without throwing errors.
    POLLER.delete(source)?;
    Ok(())
}
