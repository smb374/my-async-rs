//! My implementation of async IO runtime in Rust.
//!
//! The goal of this runtime is to provide a relatively short code
//! with clear documentation on library interface and architechture design
//! to help understanding the underlying mechanism of an asynchronous runtime.
//!
//! The crate has the following components:
//!
//! * Future implementation for [`AsFd`][d] types: You can encapsulate any type that
//! implements [`AsFd`][d] + [`Unpin`] using [`IoWrapper`].
//! * Predefined type and API for file and net operation under [`fs`] and [`net`] for convenience.
//! * [Single-threaded executor][single_thread::Executor] and [multi-threaded executor][multi_thread::Executor]
//! for executing futures.
//! * [Future scheduler][`schedulers`] for multi-thread executor. Currently implements
//! [`HybridScheduler`][a], [`WorkStealingScheduler`][b], [`RoundRobinScheduler`][c].
//! * Reactor based on [`mio`][e].
//!
//!
//!
//! [a]: schedulers::hybrid::HybridScheduler
//! [b]: schedulers::work_stealing::WorkStealingScheduler
//! [c]: schedulers::round_robin::RoundRobinScheduler
//! [d]: https://docs.rs/rustix/0.35.13/rustix/fd/trait.AsFd.html
//! [e]: https://docs.rs/mio/0.8.5/mio/index.html

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

use core::{
    convert::{AsMut, AsRef},
    pin::Pin,
    sync::atomic::{AtomicUsize, Ordering},
    task::{Context, Poll},
};
use std::{
    io::{Read, Write},
    os::fd::{AsFd, AsRawFd, BorrowedFd, RawFd},
};

use futures_lite::{future::poll_fn, AsyncRead, AsyncWrite, FutureExt};
use mio::{event::Source, unix::SourceFd, Registry, Token};

pub use mio::Interest;
pub use modules::{fs, io, net, stream};

/// Token bucket based auto task yielding implementation.
///
/// An implementation using the idea from [`tokio`'s per-task operation budget](https://tokio.rs/blog/2020-04-preemption)
/// using a token bucket like algorithm.
///
/// To enable the budget use when using this runtime, `use` this trait on top of your module
/// for overriding [`Future`][b]'s `poll` definition.
///
/// This trait is auto-implemented for all types that implements [`FutureExt`][a], which
/// itself is auto-implemented for all types implementing future [`Future`][b].
///
/// # Note
/// This trait hasn't been tested thoroughly, the trait may have little or no improvement
/// compare to normal usage.
///
/// [a]: https://docs.rs/futures-lite/1.12.0/futures_lite/future/trait.FutureExt.html
/// [b]: std::future::Future
pub trait BudgetFuture: FutureExt {
    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<Self::Output>
    where
        Self: Unpin,
    {
        poll_with_budget(self, cx)
    }
}

impl<F: FutureExt + ?Sized> BudgetFuture for F {}

/// Wrapper around [`AsFd`][a] + [`Unpin`] types.
///
/// You can use [`IoWrapper::from()`] for easy convertion over [`AsFd`] types.
///
/// There are various predefined under [`fs`] and [`net`] with convenient alias functions.
/// # Note
/// If you need to perform other operations that is not [`AsyncRead`], [`AsyncWrite`],
/// or other predefined alias functions, consider:
/// 1. Non-IO and synchronous operations: use [`IoWrapper::inner()`] for reference and
///    [`IoWrapper::inner_mut()`] for mutable reference to the wrapped type.
/// 2. Operations need `&self`: use [`IoWrapper::ref_io()`]:
/// ```ignore
/// // Read Operations
/// let result = self.ref_io(Interest::READABLE, |me| {
///     let inner = me.inner();
///     // do IO stuff on inner type...
///     result
/// }).await;
/// // Write Operations
/// let result = self.ref_io(Interest::WRITABLE, |me| {
///     let inner = me.inner();
///     // do IO stuff on inner type...
///     result
/// }).await;
/// // Both
/// let result = self.ref_io(Interest::READABLE | Interest::WRITABLE, |me| {
///     let inner = me.inner();
///     // do IO stuff on inner type...
///     result
/// }).await;
/// ```
/// 3. Operations need `&mut self`: use [`IoWrapper::mut_io()`]:
/// ```ignore
/// // Read Operations
/// let result = self.mut_io(Interest::READABLE, |me| {
///     let mut inner = me.inner_mut();
///     // do IO stuff on inner type...
///     result
/// }).await;
/// // Write Operations
/// let result = self.mut_io(Interest::WRITABLE, |me| {
///     let mut inner = me.inner_mut();
///     // do IO stuff on inner type...
///     result
/// }).await;
/// // Both
/// let result = self.mut_io(Interest::READABLE | Interest::WRITABLE, |me| {
///     let mut inner = me.inner_mut();
///     // do IO stuff on inner type...
///     result
/// }).await;
/// ```
///
/// [a]: https://docs.rs/rustix/0.35.13/rustix/fd/trait.AsFd.html
#[derive(Debug)]
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
    /// Performing async IO with `F` without self mutation.
    ///
    /// This function supports calling `f` that takes `&self`, which
    /// allows mutating external variables but self.
    ///
    /// You should specify the [`Interest`] of this IO, which is the combination
    /// of [`READABLE`][Interest::READABLE] and [`WRITABLE`][Interest::WRITABLE].
    ///
    /// For example, take a look on the implementation of
    /// [`Tcpstream::peek()`][crate::net::TcpStream::peek()]:
    ///
    /// ```ignore
    /// pub async fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
    ///     self.ref_io(Interest::READABLE, |me| me.inner().peek(buf))
    ///         .await
    /// }
    /// ```
    ///
    /// To mutate self, call [`mut_io`][IoWrapper::mut_io] instead.
    pub async fn ref_io<U, F>(&self, interest: Interest, mut f: F) -> io::Result<U>
    where
        F: FnMut(&Self) -> io::Result<U>,
    {
        poll_fn(|cx| self.poll_ref(cx, interest, &mut f)).await
    }

    /// Performing async IO with `F` with self mutation.
    ///
    /// The usage is as same as [`ref_io`][IoWrapper::ref_io], refer it for documentation.
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
