// reactor, not exposed
mod reactor;
// modules
mod modules;
// executor variant
pub mod multi_thread;
pub mod single_thread;
// scheduler
pub mod schedulers;
pub mod utils;

use crate::schedulers::poll_with_budget;

use std::{
    convert::{AsMut, AsRef},
    io::{Read, Write},
    pin::Pin,
    sync::atomic::{AtomicUsize, Ordering},
    task::{Context, Poll},
};

use futures_lite::{future::poll_fn, AsyncRead, AsyncWrite, FutureExt};
use mio::{event::Source, unix::SourceFd, Registry, Token};
use rustix::fd::{AsFd, AsRawFd, BorrowedFd, RawFd};

pub use mio::Interest;
pub use modules::{fs, io, net, stream};

pub trait BudgetFuture: FutureExt {
    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<Self::Output>
    where
        Self: Unpin,
    {
        poll_with_budget(self, cx)
    }
}

impl<F: FutureExt + ?Sized> BudgetFuture for F {}

pub struct IoWrapper<T: AsFd> {
    inner: T,
    token: AtomicUsize,
}

impl<T: AsFd> IoWrapper<T> {
    fn register_reactor(
        &self,
        current_token: usize,
        interests: Interest,
        cx: &mut Context<'_>,
    ) -> io::Result<()> {
        let waker = cx.waker().clone();
        let fd = self.as_raw_fd();
        let mut source = SourceFd(&fd);
        if let Some(token) = reactor::add_waker(current_token, waker) {
            self.token.store(token, Ordering::Relaxed);
            reactor::register(&mut source, Token(token), interests, false)?;
        } else {
            reactor::register(&mut source, Token(current_token), interests, true)?;
        }
        Ok(())
    }
    #[allow(dead_code)]
    fn degister_reactor(&self) -> io::Result<()> {
        let fd = self.as_raw_fd();
        let mut source = SourceFd(&fd);
        let current = self.token.load(Ordering::Relaxed);
        reactor::deregister(&mut source, Token(current))?;
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
            token: AtomicUsize::new(usize::MAX),
        }
    }
}

impl<T: AsFd> IoWrapper<T> {
    fn set_nonblocking(fd: &T) -> io::Result<()> {
        utils::set_nonblocking(fd)
    }
}

impl<T: AsFd + Unpin> IoWrapper<T> {
    pub async fn ref_io<U, F>(&self, interest: Interest, mut f: F) -> io::Result<U>
    where
        F: FnMut(&Self) -> io::Result<U>,
    {
        poll_fn(|cx| self.poll_ref(cx, interest, &mut f)).await
    }

    pub async fn mut_io<U, F>(&mut self, interest: Interest, mut f: F) -> io::Result<U>
    where
        F: FnMut(&mut Self) -> io::Result<U>,
    {
        poll_fn(|cx| self.poll_mut(cx, interest, &mut f)).await
    }

    fn poll_pinned<U, F>(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        interest: Interest,
        f: F,
    ) -> Poll<io::Result<U>>
    where
        F: FnMut(&mut Self) -> io::Result<U>,
    {
        let me = self.get_mut();
        me.poll_mut(cx, interest, f)
    }

    fn poll_ref<U, F>(
        &self,
        cx: &mut Context<'_>,
        interest: Interest,
        mut f: F,
    ) -> Poll<io::Result<U>>
    where
        F: FnMut(&Self) -> io::Result<U>,
    {
        match f(self) {
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                let current = self.token.load(Ordering::Relaxed);
                self.register_reactor(current, interest, cx)?;
                Poll::Pending
            }
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => self.poll_ref(cx, interest, f),
            r => Poll::Ready(r),
        }
    }

    fn poll_mut<U, F>(
        &mut self,
        cx: &mut Context<'_>,
        interest: Interest,
        mut f: F,
    ) -> Poll<io::Result<U>>
    where
        F: FnMut(&mut Self) -> io::Result<U>,
    {
        match f(self) {
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                let current = self.token.load(Ordering::Relaxed);
                self.register_reactor(current, interest, cx)?;
                Poll::Pending
            }
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => self.poll_mut(cx, interest, f),
            r => Poll::Ready(r),
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

impl<T: AsFd> Source for IoWrapper<T> {
    fn register(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        SourceFd(&self.as_raw_fd()).register(registry, token, interests)
    }

    fn reregister(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        SourceFd(&self.as_raw_fd()).reregister(registry, token, interests)
    }

    fn deregister(&mut self, registry: &Registry) -> io::Result<()> {
        SourceFd(&self.as_raw_fd()).deregister(registry)
    }
}

impl<T: AsFd + Read + Unpin> AsyncRead for IoWrapper<T> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        self.poll_pinned(cx, Interest::READABLE, |x| x.inner.read(buf))
    }
}

impl<T: AsFd + Write + Unpin> AsyncWrite for IoWrapper<T> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.poll_pinned(cx, Interest::WRITABLE, |x| x.inner.write(buf))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_pinned(cx, Interest::READABLE | Interest::WRITABLE, |x| {
            x.inner.flush()
        })
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_flush(cx)
    }
}
