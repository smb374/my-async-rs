use std::{
    io,
    ops::{Deref, DerefMut},
    time::Duration,
};

use crossbeam::sync::ShardedLock;
use futures_task::Waker;
use mio::{
    event::{Event, Source},
    Events, Interest, Poll, Registry, Token,
};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use rustc_hash::FxHashMap;

use crate::unpoison;

static REGISTRY: Lazy<ShardedLock<Option<Registry>>> = Lazy::new(|| ShardedLock::new(None));
static WAKER_MAP: Lazy<Mutex<FxHashMap<Token, WakerSet>>> =
    Lazy::new(|| Mutex::new(FxHashMap::default()));

pub struct Reactor {
    poll: Poll,
    events: Events,
    extra_wakeups: FxHashMap<Token, Event>,
}

pub struct WakerSet {
    read: Option<Waker>,
    write: Option<Waker>,
}

impl Reactor {
    pub fn new(capacity: usize) -> Self {
        let poll = Poll::new().expect("Failed to setup Poll");
        let events = Events::with_capacity(capacity);
        let extra_wakeups = FxHashMap::default();
        Self {
            poll,
            events,
            extra_wakeups,
        }
    }

    pub fn wait(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        self.poll.poll(&mut self.events, timeout)?;
        if !self.events.is_empty() {
            tracing::debug!("Start process events.");
            self.events.iter().for_each(|e| {
                let mut guard = WAKER_MAP.lock();
                let wakers_ref = guard.deref_mut();
                if let Some(ws) = wakers_ref.get_mut(&e.token()) {
                    process_waker(ws, e);
                } else {
                    self.extra_wakeups.insert(e.token(), e.clone());
                }
            });
        }
        Ok(())
    }

    pub fn setup_registry(&self) {
        let mut guard = unpoison(REGISTRY.write());
        let registry = self
            .poll
            .registry()
            .try_clone()
            .expect("Failed to clone registry");
        guard.replace(registry);
    }

    pub fn check_extra_wakeups(&mut self) -> bool {
        let mut event_checked = false;
        self.extra_wakeups.retain(|t, e| {
            let mut guard = WAKER_MAP.lock();
            let wakers_ref = guard.deref_mut();
            if let Some(ws) = wakers_ref.get_mut(t) {
                event_checked = true;
                process_waker(ws, e);
                !ws.is_empty()
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

impl WakerSet {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn is_empty(&self) -> bool {
        self.read.is_none() && self.write.is_none()
    }
}

impl Default for WakerSet {
    fn default() -> Self {
        Self {
            read: None,
            write: None,
        }
    }
}

pub(crate) fn add_waker(token: Token, interests: Interest, waker: Waker) -> io::Result<()> {
    let mut guard = WAKER_MAP.lock();
    let lock = guard.deref_mut();
    if let Some(ws) = lock.get_mut(&token) {
        if interests.is_readable() {
            if let Some(w) = ws.read.replace(waker) {
                w.wake_by_ref();
            }
        } else if interests.is_writable() {
            if let Some(w) = ws.write.replace(waker) {
                w.wake_by_ref();
            }
        }
    } else {
        let mut ws = WakerSet::new();
        if interests.is_readable() {
            ws.read.replace(waker);
        } else if interests.is_writable() {
            ws.write.replace(waker);
        }
        lock.insert(token, ws);
    }
    Ok(())
}

pub(crate) fn remove_waker(token: Token) {
    let mut guard = WAKER_MAP.lock();
    let lock = guard.deref_mut();
    lock.remove(&token);
}

pub fn register<S>(source: &mut S, token: Token, interests: Interest) -> io::Result<()>
where
    S: Source + ?Sized,
{
    let registry_guard = unpoison(REGISTRY.read());
    if let Some(registry) = registry_guard.deref() {
        let mut wakers_guard = WAKER_MAP.lock();
        let wakers_ref = wakers_guard.deref_mut();
        if wakers_ref.get(&token).is_some() {
            registry.reregister(source, token, interests)?;
        } else {
            registry.register(source, token, interests)?;
        }
    }
    Ok(())
}

pub fn deregister<S>(_source: &mut S, token: Token) -> io::Result<()>
where
    S: Source + ?Sized,
{
    remove_waker(token);
    // TODO: find a way to deregister without throwing errors.
    // let registry_guard = REGISTRY.lock();
    // if let Some(_registry) = registry_guard.deref() {
    //     registry.deregister(source)?;
    // }
    Ok(())
}

fn process_waker(ws: &mut WakerSet, e: &Event) {
    if e.is_readable() {
        if let Some(w) = ws.read.take() {
            w.wake_by_ref();
        }
    }
    if e.is_writable() {
        if let Some(w) = ws.write.take() {
            w.wake_by_ref();
        }
    }
}
