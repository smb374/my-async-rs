// reactor, not exposed
mod reactor;
// modules
mod modules;
// executor variant
pub mod multi_thread;
pub mod single_thread;
// scheduler
pub mod schedulers;

use std::{
    convert::{AsMut, AsRef},
    io::{Read, Write},
    pin::Pin,
    sync::atomic::{AtomicUsize, Ordering},
    task::{Context, Poll},
};

use futures_lite::{future::poll_fn, AsyncRead, AsyncWrite};
use polling::Event;
use rustix::{
    fd::{AsFd, AsRawFd, BorrowedFd, RawFd},
    fs::{fcntl_getfl, fcntl_setfl, OFlags},
};

pub use modules::{fs, io, net, stream};

pub enum IoType {
    Read,
    Write,
    Both,
}

pub struct IoWrapper<T: AsFd> {
    inner: T,
    key: AtomicUsize,
}

impl IoType {
    fn to_event(&self, key: usize) -> Event {
        match *self {
            IoType::Read => Event::readable(key),
            IoType::Write => Event::writable(key),
            IoType::Both => Event::all(key),
        }
    }
}

impl<T: AsFd> IoWrapper<T> {
    pub(crate) fn register_reactor(&self, interest: Event, cx: &mut Context<'_>) -> io::Result<()> {
        let waker = cx.waker().clone();
        let current = interest.key;
        if let Some(key) = reactor::add_waker(current, waker) {
            self.key.store(key, Ordering::Relaxed);
            let new_interest = Event {
                key,
                readable: interest.readable,
                writable: interest.writable,
            };
            reactor::register(self, new_interest, false)?;
        } else {
            reactor::register(self, interest, true)?;
        }
        Ok(())
    }
    #[allow(dead_code)]
    pub(crate) fn degister_reactor(&self) -> io::Result<()> {
        let current = self.key.load(Ordering::Relaxed);
        reactor::deregister(self, current)?;
        Ok(())
    }
    pub fn inner(&self) -> &T {
        &self.inner
    }
    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<T: AsFd> From<T> for IoWrapper<T> {
    fn from(inner: T) -> Self {
        Self::set_nonblocking(&inner).expect("Failed to set nonblocking");
        Self {
            inner,
            key: AtomicUsize::new(usize::MAX),
        }
    }
}

impl<T: AsFd> IoWrapper<T> {
    fn set_nonblocking(fd: &T) -> io::Result<()> {
        let mut current_flags =
            fcntl_getfl(fd).map_err(|e| io::Error::from_raw_os_error(e.raw_os_error()))?;
        if !current_flags.contains(OFlags::NONBLOCK) {
            current_flags.set(OFlags::NONBLOCK, true);
            fcntl_setfl(fd, current_flags)
                .map_err(|e| io::Error::from_raw_os_error(e.raw_os_error()))?;
        }
        Ok(())
    }
}

impl<T: AsFd + Unpin> IoWrapper<T> {
    pub async fn io_ref<U, F>(&self, io_type: IoType, mut f: F) -> io::Result<U>
    where
        F: FnMut(&Self) -> io::Result<U>,
    {
        let interest = io_type.to_event(self.key.load(Ordering::Relaxed));
        self.ref_io(interest, &mut f).await
    }

    pub async fn io_mut<U, F>(&mut self, io_type: IoType, mut f: F) -> io::Result<U>
    where
        F: FnMut(&mut Self) -> io::Result<U>,
    {
        let interest = io_type.to_event(self.key.load(Ordering::Relaxed));
        self.mut_io(interest, &mut f).await
    }

    async fn ref_io<U, F>(&self, interest: Event, mut f: F) -> io::Result<U>
    where
        F: FnMut(&Self) -> io::Result<U>,
    {
        poll_fn(|cx| self.poll_ref(cx, interest, &mut f)).await
    }

    async fn mut_io<U, F>(&mut self, interest: Event, mut f: F) -> io::Result<U>
    where
        F: FnMut(&mut Self) -> io::Result<U>,
    {
        poll_fn(|cx| self.poll_mut(cx, interest, &mut f)).await
    }

    fn poll_pinned<U, F>(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        interest: Event,
        mut f: F,
    ) -> Poll<io::Result<U>>
    where
        F: FnMut(&mut Self) -> io::Result<U>,
    {
        let me = self.get_mut();
        match f(me) {
            Ok(r) => Poll::Ready(Ok(r)),
            Err(e) => match e.kind() {
                io::ErrorKind::WouldBlock => {
                    me.register_reactor(interest, cx)?;
                    Poll::Pending
                }
                io::ErrorKind::Interrupted => Pin::new(me).poll_pinned(cx, interest, f),
                _ => Poll::Ready(Err(e)),
            },
        }
    }

    fn poll_ref<U, F>(&self, cx: &mut Context<'_>, interest: Event, mut f: F) -> Poll<io::Result<U>>
    where
        F: FnMut(&Self) -> io::Result<U>,
    {
        match f(self) {
            Ok(r) => Poll::Ready(Ok(r)),
            Err(e) => match e.kind() {
                io::ErrorKind::WouldBlock => {
                    self.register_reactor(interest, cx)?;
                    Poll::Pending
                }
                io::ErrorKind::Interrupted => self.poll_ref(cx, interest, f),
                _ => Poll::Ready(Err(e)),
            },
        }
    }

    fn poll_mut<U, F>(
        &mut self,
        cx: &mut Context<'_>,
        interest: Event,
        mut f: F,
    ) -> Poll<io::Result<U>>
    where
        F: FnMut(&mut Self) -> io::Result<U>,
    {
        match f(self) {
            Ok(r) => Poll::Ready(Ok(r)),
            Err(e) => match e.kind() {
                io::ErrorKind::WouldBlock => {
                    self.register_reactor(interest, cx)?;
                    Poll::Pending
                }
                io::ErrorKind::Interrupted => self.poll_mut(cx, interest, f),
                _ => Poll::Ready(Err(e)),
            },
        }
    }
}

impl<T: AsFd> AsRef<T> for IoWrapper<T> {
    fn as_ref(&self) -> &T {
        self.inner()
    }
}

impl<T: AsFd> AsMut<T> for IoWrapper<T> {
    fn as_mut(&mut self) -> &mut T {
        self.inner_mut()
    }
}

impl<T: AsFd> AsFd for IoWrapper<T> {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.inner.as_fd()
    }
}

impl<T: AsFd> AsRawFd for IoWrapper<T> {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_fd().as_raw_fd()
    }
}

impl<T: AsFd + Read + Unpin> AsyncRead for IoWrapper<T> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let key = self.key.load(Ordering::Relaxed);
        self.poll_pinned(cx, Event::readable(key), |x| x.inner.read(buf))
    }
}

impl<T: AsFd + Write + Unpin> AsyncWrite for IoWrapper<T> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let key = self.key.load(Ordering::Relaxed);
        self.poll_pinned(cx, Event::writable(key), |x| x.inner.write(buf))
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let key = self.key.load(Ordering::Relaxed);
        self.poll_pinned(cx, Event::writable(key), |x| x.inner.flush())
    }
    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_flush(cx)
    }
}
