use crate::{impl_common_write, Interest, IoWrapper};

use std::{
    future::Future,
    io::{self, Write},
    net::{SocketAddr, ToSocketAddrs},
    pin::Pin,
    task::{Context, Poll},
};

use futures_lite::{io::AsyncWrite, Stream};
use tracing::error;

pub type TcpStream = IoWrapper<std::net::TcpStream>;
pub type TcpListener = IoWrapper<std::net::TcpListener>;
// pub type UdpSocket = IoWrapper<std::net::UdpSocket>;

pub struct Incoming<'a> {
    listener: &'a mut TcpListener,
}

pub struct Accept<'a> {
    listener: Pin<&'a mut TcpListener>,
}

pub struct Peek<'a> {
    stream: Pin<&'a mut TcpStream>,
    buf: &'a mut [u8],
}

impl TcpStream {
    pub fn connect<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        let stdstream = std::net::TcpStream::connect(addr)?;
        Ok(IoWrapper::from(stdstream))
    }
    pub fn peek<'a>(&'a mut self, buf: &'a mut [u8]) -> Peek<'a> {
        Peek {
            stream: Pin::new(self),
            buf,
        }
    }
    fn poll_peek(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let me = self.get_mut();
        match me.inner.peek(buf) {
            Ok(n) => Poll::Ready(Ok(n)),
            Err(e) => match e.kind() {
                io::ErrorKind::WouldBlock => {
                    me.register_reactor(Interest::READABLE, cx)?;
                    Poll::Pending
                }
                io::ErrorKind::Interrupted => Pin::new(me).poll_peek(cx, buf),
                _ => Poll::Ready(Err(e)),
            },
        }
    }
}

impl AsyncWrite for TcpStream {
    impl_common_write!();
    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let me = self.get_mut();
        me.inner.shutdown(std::net::Shutdown::Both)?;
        Pin::new(me).poll_flush(cx)
    }
}

impl TcpListener {
    pub fn incoming(&mut self) -> Incoming<'_> {
        Incoming { listener: self }
    }
    pub fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        let inner = std::net::TcpListener::bind(addr)?;
        Ok(IoWrapper::from(inner))
    }
    pub fn accept(&mut self) -> Accept<'_> {
        Accept {
            listener: Pin::new(self),
        }
    }
    pub fn poll_accept(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<(TcpStream, SocketAddr)>> {
        let me = self.get_mut();
        match me.as_ref().accept() {
            Ok((stream, addr)) => {
                let wrapped_stream = IoWrapper::from(stream);
                Poll::Ready(Ok((wrapped_stream, addr)))
            }
            Err(e) => match e.kind() {
                io::ErrorKind::WouldBlock => {
                    me.register_reactor(Interest::READABLE, cx)?;
                    Poll::Pending
                }
                io::ErrorKind::Interrupted => Pin::new(me).poll_accept(cx),
                _ => Poll::Ready(Err(e)),
            },
        }
    }
}

impl<'a> Future for Accept<'a> {
    type Output = io::Result<(TcpStream, SocketAddr)>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.get_mut();
        me.listener.as_mut().poll_accept(cx)
    }
}

impl<'a> Future for Peek<'a> {
    type Output = io::Result<usize>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.get_mut();
        me.stream.as_mut().poll_peek(cx, me.buf)
    }
}

impl<'a> Stream for Incoming<'a> {
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
                    error!("Failed to accept new connection: {}", e);
                    Poll::Ready(None)
                }
            },
        }
    }
}
