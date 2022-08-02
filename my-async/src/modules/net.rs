use crate::{impl_common_write, Interest, IoWrapper};

use std::{
    io::{self, Write},
    net::{SocketAddr, ToSocketAddrs},
    path::Path,
    pin::Pin,
    task::{Context, Poll},
};

use futures_lite::{io::AsyncWrite, Stream};

// Net Socket
pub type TcpStream = IoWrapper<std::net::TcpStream>;
pub type TcpListener = IoWrapper<std::net::TcpListener>;
pub type UdpSocket = IoWrapper<std::net::UdpSocket>;

pub struct TcpIncoming<'a> {
    listener: &'a mut TcpListener,
}

// UDS
pub type UnixStream = IoWrapper<std::os::unix::net::UnixStream>;
pub type UnixListener = IoWrapper<std::os::unix::net::UnixListener>;
pub type UnixDatagram = IoWrapper<std::os::unix::net::UnixDatagram>;

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

impl AsyncWrite for TcpStream {
    impl_common_write!();
    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_flush(cx)
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
                    me.listener.register_reactor(Interest::READABLE, cx)?;
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

impl AsyncWrite for UnixStream {
    impl_common_write!();
    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_flush(cx)
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
                    me.listener.register_reactor(Interest::READABLE, cx)?;
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
