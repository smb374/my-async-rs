use std::{io, task::Waker, time::Duration};

use mio::{event::Source, Events, Interest, Poll, Registry, Token};
use once_cell::sync::{Lazy, OnceCell};
use parking_lot::{Mutex, MutexGuard};
use sharded_slab::Slab;

static REGISTRY: OnceCell<Registry> = OnceCell::new();
static WAKER_SLAB: Lazy<Slab<Mutex<Option<Waker>>>> = Lazy::new(Slab::new);
static POLL_WAKE_TOKEN: Token = Token(usize::MAX);
pub(super) static POLL_WAKER: OnceCell<mio::Waker> = OnceCell::new();

pub struct Reactor {
    poll: Poll,
    events: Events,
    extra_wakeups: Vec<usize>,
}

impl Reactor {
    pub fn new(capacity: usize) -> Self {
        let poll = Poll::new().expect("Failed to setup Poll");
        let events = Events::with_capacity(capacity);
        let extra_wakeups = Vec::with_capacity(1024);
        Self {
            poll,
            events,
            extra_wakeups,
        }
    }

    pub fn wait(&mut self, timeout: Option<Duration>) -> io::Result<bool> {
        self.poll.poll(&mut self.events, timeout)?;
        if !self.events.is_empty() {
            log::debug!("Start process events.");
            for e in self.events.iter() {
                if e.token() == POLL_WAKE_TOKEN {
                    return Ok(true);
                }
                let idx = e.token().0;
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
        Ok(false)
    }

    pub fn setup_registry(&self) {
        let registry = self
            .poll
            .registry()
            .try_clone()
            .expect("Failed to clone registry");
        POLL_WAKER.get_or_init(|| match mio::Waker::new(&registry, POLL_WAKE_TOKEN) {
            Ok(waker) => waker,
            Err(e) => panic!("Failed to setup waker for poll: {}", e),
        });
        REGISTRY.get_or_init(move || registry);
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

pub(crate) fn add_waker(token: usize, waker: Waker) -> Option<usize> {
    let waker_found = process_waker(token, |guard| {
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

pub(crate) fn remove_waker(token: Token) -> bool {
    WAKER_SLAB.remove(token.0)
}

pub fn register<S>(
    source: &mut S,
    token: Token,
    interests: Interest,
    reregister: bool,
) -> io::Result<()>
where
    S: Source + ?Sized,
{
    if let Some(registry) = REGISTRY.get() {
        if reregister {
            registry.reregister(source, token, interests)?;
        } else {
            registry.register(source, token, interests)?;
        }
    } else {
        log::error!("Registry hasn't initialized.")
    }
    Ok(())
}

pub fn deregister<S>(source: &mut S, token: Token) -> io::Result<()>
where
    S: Source + ?Sized,
{
    remove_waker(token);
    // TODO: find a way to deregister without throwing errors.
    if let Some(registry) = REGISTRY.get() {
        registry.deregister(source)?;
    }
    Ok(())
}
