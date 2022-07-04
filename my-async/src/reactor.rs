use std::{io, task::Waker, time::Duration};

use mio::{event::Source, Events, Interest, Poll, Registry, Token};
use once_cell::sync::{Lazy, OnceCell};
use parking_lot::Mutex;
use sharded_slab::Slab;

static REGISTRY: OnceCell<Registry> = OnceCell::new();
static WAKER_SLAB: Lazy<Slab<Mutex<Option<Waker>>>> = Lazy::new(Slab::new);

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

    pub fn wait(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        self.poll.poll(&mut self.events, timeout)?;
        if !self.events.is_empty() {
            log::debug!("Start process events.");
            self.events.iter().for_each(|e| {
                if WAKER_SLAB.contains(e.token().0) {
                    let mutex = WAKER_SLAB.get(e.token().0).unwrap();
                    let mut guard = mutex.lock();
                    if let Some(w) = guard.take() {
                        w.wake_by_ref();
                    }
                } else {
                    self.extra_wakeups.push(e.token().0);
                }
            });
        }
        Ok(())
    }

    pub fn setup_registry(&self) {
        let registry = self
            .poll
            .registry()
            .try_clone()
            .expect("Failed to clone registry");
        REGISTRY.get_or_init(move || registry);
    }

    pub fn check_extra_wakeups(&mut self) -> bool {
        let mut event_checked = false;
        self.extra_wakeups.retain(|&idx| {
            if WAKER_SLAB.contains(idx) {
                let mutex = WAKER_SLAB.get(idx).unwrap();
                let mut guard = mutex.lock();
                if let Some(w) = guard.take() {
                    event_checked = true;
                    w.wake_by_ref();
                }
                false
            } else {
                true
            }
        });
        event_checked
    }
}

impl Default for Reactor {
    fn default() -> Self {
        Self::new(1024)
    }
}

pub(crate) fn add_waker(token: &Token, waker: Waker) -> Option<Token> {
    if WAKER_SLAB.contains(token.0) {
        let mutex = WAKER_SLAB.get(token.0).unwrap();
        let mut guard = mutex.lock();
        if let Some(w) = guard.replace(waker) {
            w.wake_by_ref();
        }
        None
    } else {
        WAKER_SLAB
            .insert(Mutex::new(Some(waker)))
            .map(|idx| Token(idx))
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
