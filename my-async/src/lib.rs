use std::{
    convert::{AsMut, AsRef},
    io::Read,
    os::unix::prelude::{AsRawFd, RawFd},
    pin::Pin,
    task::{Context, Poll},
    time::SystemTime,
};

use futures_lite::AsyncRead;
use mio::{event::Source, unix::SourceFd, Registry, Token};
use nix::fcntl::{fcntl, FcntlArg, OFlag};

// reactor, not exposed
mod reactor;
// modules
mod modules;
// executor variant
pub mod multi_thread;
pub mod single_thread;
// scheduler
pub mod schedulers;

pub use mio::Interest;
pub use modules::{fs, io, net, stream};

pub struct IoWrapper<T> {
    inner: T,
    token: Token,
}

impl<T: AsRawFd> IoWrapper<T> {
    pub fn register_reactor(&mut self, interest: Interest, cx: &mut Context<'_>) -> io::Result<()> {
        let waker = cx.waker().clone();
        reactor::register(self, self.token, interest)?;
        reactor::add_waker(self.token, interest, waker)?;
        Ok(())
    }
    pub fn degister_reactor(&mut self) -> io::Result<()> {
        reactor::deregister(self, self.token)?;
        Ok(())
    }
}

impl<T: AsRawFd> From<T> for IoWrapper<T> {
    fn from(inner: T) -> Self {
        Self::set_nonblocking(inner.as_raw_fd()).expect("Failed to set nonblocking");
        let token = token_from_unixtime();
        Self { inner, token }
    }
}

impl<T: AsRawFd> IoWrapper<T> {
    fn set_nonblocking(fd: RawFd) -> io::Result<()> {
        let mut current_flags = OFlag::from_bits_truncate(fcntl(fd, FcntlArg::F_GETFL)?);
        if !current_flags.contains(OFlag::O_NONBLOCK) {
            current_flags.set(OFlag::O_NONBLOCK, true);
            fcntl(fd, FcntlArg::F_SETFL(current_flags))?;
        }
        Ok(())
    }
}

impl<T> AsRef<T> for IoWrapper<T> {
    fn as_ref(&self) -> &T {
        &self.inner
    }
}

impl<T> AsMut<T> for IoWrapper<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<T: AsRawFd> AsRawFd for IoWrapper<T> {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

impl<T: AsRawFd> Source for IoWrapper<T> {
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

impl<T: AsRawFd + Read + Unpin> AsyncRead for IoWrapper<T> {
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

pub(crate) fn token_from_unixtime() -> Token {
    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("SystemTime before UNIX_EPOCH!");
    let time_bytes = current_time.as_nanos().to_be_bytes();
    let tail = &time_bytes[time_bytes.len() - 8..];
    let id = usize::from_be_bytes(tail.try_into().unwrap());
    Token(id)
}
