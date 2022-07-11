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
    io::Read,
    pin::Pin,
    task::{Context, Poll},
};

use futures_lite::AsyncRead;
use mio::{event::Source, unix::SourceFd, Registry, Token};
use rustix::{
    fd::{AsFd, AsRawFd, BorrowedFd, RawFd},
    fs::{fcntl_getfl, fcntl_setfl, OFlags},
};

pub use mio::Interest;
pub use modules::{fs, io, net, stream};

pub struct IoWrapper<T: AsFd> {
    inner: T,
    token: Token,
}

impl<T: AsFd> IoWrapper<T> {
    pub fn register_reactor(
        &mut self,
        interests: Interest,
        cx: &mut Context<'_>,
    ) -> io::Result<()> {
        let waker = cx.waker().clone();
        if let Some(token) = reactor::add_waker(&self.token, waker) {
            self.token = token;
            reactor::register(self, self.token, interests, false)?;
        } else {
            reactor::register(self, self.token, interests, true)?;
        }
        Ok(())
    }
    pub fn degister_reactor(&mut self) -> io::Result<()> {
        reactor::deregister(self, self.token)?;
        Ok(())
    }
}

impl<T: AsFd> From<T> for IoWrapper<T> {
    fn from(inner: T) -> Self {
        Self::set_nonblocking(&inner).expect("Failed to set nonblocking");
        Self {
            inner,
            token: Token(usize::MAX),
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

impl<T: AsFd> AsRef<T> for IoWrapper<T> {
    fn as_ref(&self) -> &T {
        &self.inner
    }
}

impl<T: AsFd> AsMut<T> for IoWrapper<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.inner
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
        // Unpin self, will move self's value
        let me = self.get_mut();
        match me.inner.read(buf) {
            Err(e) => match e.kind() {
                // Pin self again and retry.
                io::ErrorKind::Interrupted => Pin::new(me).poll_read(cx, buf),
                // Register self to reactor and wait.
                io::ErrorKind::WouldBlock => {
                    me.register_reactor(Interest::READABLE, cx)?;
                    Poll::Pending
                }
                // Other errors are returned directly.
                _ => Poll::Ready(Err(e)),
            },
            // Success, return result.
            Ok(i) => Poll::Ready(Ok(i)),
        }
    }
}

#[macro_export]
macro_rules! impl_common_write {
    () => {
        fn poll_write(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            // Unpin self, will move self's value
            let me = self.get_mut();
            match me.inner.write(buf) {
                Err(e) => match e.kind() {
                    // Pin self again and retry.
                    io::ErrorKind::Interrupted => Pin::new(me).poll_write(cx, buf),
                    // Register self to reactor and wait.
                    io::ErrorKind::WouldBlock => {
                        me.register_reactor(Interest::WRITABLE, cx)?;
                        Poll::Pending
                    }
                    // Other errors are returned directly.
                    _ => Poll::Ready(Err(e)),
                },
                // Success, return result.
                Ok(i) => Poll::Ready(Ok(i)),
            }
        }
        fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            // Unpin self, will move self's value
            let me = self.get_mut();
            match me.inner.flush() {
                Err(e) => match e.kind() {
                    // Pin self again and retry.
                    io::ErrorKind::Interrupted => Pin::new(me).poll_flush(cx),
                    // Register self to reactor and wait.
                    io::ErrorKind::WouldBlock => {
                        me.register_reactor(Interest::WRITABLE, cx)?;
                        Poll::Pending
                    }
                    // Other errors are returned directly.
                    _ => Poll::Ready(Err(e)),
                },
                // Success, return result.
                Ok(i) => Poll::Ready(Ok(i)),
            }
        }
    };
}
