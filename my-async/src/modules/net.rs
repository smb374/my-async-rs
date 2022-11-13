//! Convenient alias for types under [`std::net`].
//!
//! The module contains predefined operations for creating and operating on these types.

use crate::{Interest, IoWrapper};

use std::{
    io,
    net::{SocketAddr, ToSocketAddrs},
    path::Path,
    pin::Pin,
    sync::atomic::Ordering,
    task::{Context, Poll},
};

use futures_lite::Stream;

// Net Socket
/// [`TcpStream`][std::net::TcpStream] wrapper type.
///
/// This type implements:
/// - [`connect()`][TcpStream::connect()]: For convenient `IoWrapper<std::net::TcpStream>`
/// creation. See [`std::net::TcpStream::connect()`].
/// - [`peek()`][TcpStream::peek()]: Async [`std::net::TcpStream::peek()`].
///
/// For other operations, refer to [`IoWrapper`'s documentation][IoWrapper].
pub type TcpStream = IoWrapper<std::net::TcpStream>;
/// [`TcpListener`][std::net::TcpListener] wrapper type.
///
/// This type implements:
/// - [`bind()`][TcpListener::bind()]: For convenient `IoWrapper<std::net::TcpListener>` bind
/// address. See [`std::net::TcpListener::bind()`].
/// - [`incoming()`][TcpListener::bind()]: Returns [`TcpIncoming`] which implements [`Stream`]
/// for accepting loop.
/// - [`accept()`][TcpListener::accept()]: Async [`std::net::TcpListener::accept()`].
///
/// For other operations, refer to [`IoWrapper`'s documentation][IoWrapper].
pub type TcpListener = IoWrapper<std::net::TcpListener>;
/// [`UdpSocket`][std::net::UdpSocket] wrapper type.
///
/// This type implements:
/// - [`bind()`][UdpSocket::bind()]: For convenient `IoWrapper<std::net::UdpSocket>` bind
/// address. See [`std::net::UdpSocket::bind()`].
/// - [`connect()`][UdpSocket::connect()]: Async [`std::net::UdpSocket::connect()`].
/// - [`peek()`][UdpSocket::peek()]: Async [`std::net::UdpSocket::peek()`].
/// - [`peek_from()`][UdpSocket::peek_from()]: Async [`std::net::UdpSocket::peek_from()`].
/// - [`recv()`][UdpSocket::recv()]: Async [`std::net::UdpSocket::recv()`].
/// - [`recv_from()`][UdpSocket::recv_from()]: Async [`std::net::UdpSocket::recv_from()`].
/// - [`send()`][UdpSocket::send()]: Async [`std::net::UdpSocket::send()`].
/// - [`send_to()`][UdpSocket::send_to()]: Async [`std::net::UdpSocket::send_to()`].
///
/// For other operations, refer to [`IoWrapper`'s documentation][IoWrapper].
pub type UdpSocket = IoWrapper<std::net::UdpSocket>;

/// [`Stream`] implmentation for [`TcpListener::incoming()`][a].
///
/// This struct implements [`Stream`] allowing to use it like an iterator
/// for looping needs.
///
/// # Example
/// ```
/// // example usage for incoming() in a accept loop.
/// async fn server() -> io::Result<()> {
///     let mut listener = TcpListener::bind("127.0.0.1:6699")?;
///     let mut incoming = listener.incoming();
///     // Using incoming in a while-let loop for convenience.
///     while let Ok(next) = incoming.try_next().await {
///         match next {
///             Some((stream, _)) => {
///                 // spawn a handler for stream
///                 let _ = spawn(handler(stream));
///             }
///             None => {
///                 break;
///             }
///         }
///     }
///     Ok(())
/// }
/// ```
///
/// [a]: std::net::TcpListener::incoming()
pub struct TcpIncoming<'a> {
    listener: &'a mut TcpListener,
}

// UDS
/// [`UnixStream`][std::os::unix::net::UnixStream] wrapper type.
///
/// This type implements:
/// - [`connect()`][UnixStream::connect()]: For convenient `IoWrapper<std::os::unix::net::UnixStream>`
/// creation. See [`std::os::unix::net::UnixStream::connect()`].
/// - [`pair()`][UnixStream::pair()]: For convenient
/// `IoWrapper<std::os::unix::net::UnixStream>` pair creation. See [`std::os::unix::net::UnixStream::pair()`].
///
/// For other operations, refer to [`IoWrapper`'s documentation][IoWrapper].
pub type UnixStream = IoWrapper<std::os::unix::net::UnixStream>;
/// [`UnixListener`][std::os::unix::net::UnixListener] wrapper type.
///
/// This type implements:
/// - [`bind()`][UnixListener::bind()]: For convenient `IoWrapper<std::os::unix::net::UnixListener>` bind
/// address. See [`std::os::unix::net::UnixListener::bind()`].
/// - [`incoming()`][UnixListener::bind()]: Returns [`UnixIncoming`] which implements [`Stream`]
/// for accepting loop.
/// - [`accept()`][UnixListener::accept()]: Async [`std::os::unix::net::UnixListener::accept()`].
///
/// For other operations, refer to [`IoWrapper`'s documentation][IoWrapper].
pub type UnixListener = IoWrapper<std::os::unix::net::UnixListener>;
/// [`UnixDatagram`][std::os::unix::net::UnixDatagram] wrapper type.
///
/// This type implements:
/// - [`bind()`][UnixDatagram::bind()]: For convenient `IoWrapper<std::os::unix::net::UnixDatagram>` bind
/// address. See [`std::os::unix::net::UnixDatagram::bind()`].
/// - [`pair()`][UnixDatagram::pair()]: For convenient
/// `IoWrapper<std::os::unix::net::UnixDatagram>` pair creation. See [`std::os::unix::net::UnixDatagram::pair()`].
/// - [`connect()`][UnixDatagram::connect()]: Async [`std::os::unix::net::UnixDatagram::connect()`].
/// - [`recv()`][UnixDatagram::recv()]: Async [`std::os::unix::net::UnixDatagram::recv()`].
/// - [`recv_from()`][UnixDatagram::recv_from()]: Async [`std::os::unix::net::UnixDatagram::recv_from()`].
/// - [`send()`][UnixDatagram::send()]: Async [`std::os::unix::net::UnixDatagram::send()`].
/// - [`send_to()`][UnixDatagram::send_to()]: Async [`std::os::unix::net::UnixDatagram::send_to()`].
///
/// For other operations, refer to [`IoWrapper`'s documentation][IoWrapper].
pub type UnixDatagram = IoWrapper<std::os::unix::net::UnixDatagram>;

/// [`Stream`] implmentation for [`UnixListener::incoming()`][a].
///
/// This struct implements [`Stream`] allowing to use it like an iterator
/// for looping needs.
///
/// See [`TcpIncoming`] for example.
///
/// [a]: std::os::unix::net::UnixListener::incoming()
pub struct UnixIncoming<'a> {
    listener: &'a mut UnixListener,
}

impl TcpStream {
    pub fn connect<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        let stdstream = std::net::TcpStream::connect(addr)?;
        Ok(IoWrapper::from(stdstream))
    }
    pub async fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.ref_io(Interest::READABLE, |me| me.inner().peek(buf))
            .await
    }
}

impl TcpListener {
    pub fn incoming(&mut self) -> TcpIncoming<'_> {
        TcpIncoming { listener: self }
    }
    pub fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        let inner = std::net::TcpListener::bind(addr)?;
        Ok(IoWrapper::from(inner))
    }
    pub async fn accept(&self) -> io::Result<(TcpStream, SocketAddr)> {
        self.ref_io(Interest::READABLE, |me| me.inner().accept())
            .await
            .map(|(s, a)| (IoWrapper::from(s), a))
    }
}

impl UdpSocket {
    pub fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<UdpSocket> {
        let inner = std::net::UdpSocket::bind(addr)?;
        Ok(IoWrapper::from(inner))
    }

    pub async fn connect<A: ToSocketAddrs + Unpin>(&self, addr: A) -> io::Result<()> {
        self.ref_io(Interest::READABLE | Interest::WRITABLE, |me| {
            me.inner().connect(&addr)
        })
        .await
    }

    pub async fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.ref_io(Interest::READABLE, |me| me.inner().peek(buf))
            .await
    }

    pub async fn peek_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.ref_io(Interest::READABLE, |me| me.inner().peek_from(buf))
            .await
    }

    pub async fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.ref_io(Interest::READABLE, |me| me.inner().recv(buf))
            .await
    }

    pub async fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.ref_io(Interest::READABLE, |me| me.inner().recv_from(buf))
            .await
    }

    pub async fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.ref_io(Interest::WRITABLE, |me| me.inner().send(buf))
            .await
    }

    pub async fn send_to<A: ToSocketAddrs + Unpin>(
        &self,
        buf: &[u8],
        addr: A,
    ) -> io::Result<usize> {
        self.ref_io(Interest::WRITABLE, |me| me.inner().send_to(buf, &addr))
            .await
    }
}

impl<'a> Stream for TcpIncoming<'a> {
    type Item = io::Result<(TcpStream, SocketAddr)>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let me = self.get_mut();
        match me.listener.as_ref().accept() {
            Ok((stream, addr)) => {
                let wrapped_stream = IoWrapper::from(stream);
                Poll::Ready(Some(Ok((wrapped_stream, addr))))
            }
            Err(e) => match e.kind() {
                io::ErrorKind::WouldBlock => {
                    let current = me.listener.token.load(Ordering::Relaxed);
                    me.listener
                        .register_reactor(current, Interest::READABLE, cx)?;
                    Poll::Pending
                }
                io::ErrorKind::Interrupted => Pin::new(me).poll_next(cx),
                _ => {
                    log::error!("Failed to accept new connection: {}", e);
                    Poll::Ready(None)
                }
            },
        }
    }
}

impl UnixListener {
    pub fn incoming(&mut self) -> UnixIncoming<'_> {
        UnixIncoming { listener: self }
    }
    pub fn bind<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let inner = std::os::unix::net::UnixListener::bind(path)?;
        Ok(IoWrapper::from(inner))
    }
    pub async fn accept(&self) -> io::Result<(UnixStream, std::os::unix::net::SocketAddr)> {
        self.ref_io(Interest::READABLE, |me| me.inner().accept())
            .await
            .map(|(s, a)| (IoWrapper::from(s), a))
    }
}

impl UnixStream {
    pub fn connect<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let inner = std::os::unix::net::UnixStream::connect(path)?;
        Ok(IoWrapper::from(inner))
    }

    pub fn pair() -> io::Result<(Self, Self)> {
        std::os::unix::net::UnixStream::pair()
            .map(|(a, b)| (IoWrapper::from(a), IoWrapper::from(b)))
    }
}

impl UnixDatagram {
    pub fn bind<P: AsRef<Path>>(path: P) -> io::Result<UnixDatagram> {
        let inner = std::os::unix::net::UnixDatagram::bind(path)?;
        Ok(IoWrapper::from(inner))
    }

    pub fn pair() -> io::Result<(Self, Self)> {
        std::os::unix::net::UnixDatagram::pair()
            .map(|(a, b)| (IoWrapper::from(a), IoWrapper::from(b)))
    }

    pub async fn connect<P: AsRef<Path> + Unpin>(&self, path: P) -> io::Result<()> {
        self.ref_io(Interest::READABLE | Interest::WRITABLE, |me| {
            me.inner().connect(path.as_ref())
        })
        .await
    }

    pub async fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.ref_io(Interest::READABLE, |me| me.inner().recv(buf))
            .await
    }

    pub async fn recv_from(
        &self,
        buf: &mut [u8],
    ) -> io::Result<(usize, std::os::unix::net::SocketAddr)> {
        self.ref_io(Interest::READABLE, |me| me.inner().recv_from(buf))
            .await
    }

    pub async fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.ref_io(Interest::WRITABLE, |me| me.inner().send(buf))
            .await
    }

    pub async fn send_to<P: AsRef<Path> + Unpin>(&self, buf: &[u8], path: P) -> io::Result<usize> {
        self.ref_io(Interest::WRITABLE, |me| {
            me.inner().send_to(buf, path.as_ref())
        })
        .await
    }
}

impl<'a> Stream for UnixIncoming<'a> {
    type Item = io::Result<(UnixStream, std::os::unix::net::SocketAddr)>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let me = self.get_mut();
        match me.listener.as_ref().accept() {
            Ok((stream, addr)) => {
                let wrapped_stream = IoWrapper::from(stream);
                Poll::Ready(Some(Ok((wrapped_stream, addr))))
            }
            Err(e) => match e.kind() {
                io::ErrorKind::WouldBlock => {
                    let current = me.listener.token.load(Ordering::Relaxed);
                    me.listener
                        .register_reactor(current, Interest::READABLE, cx)?;
                    Poll::Pending
                }
                io::ErrorKind::Interrupted => Pin::new(me).poll_next(cx),
                _ => {
                    log::error!("Failed to accept new connection: {}", e);
                    Poll::Ready(None)
                }
            },
        }
    }
}
