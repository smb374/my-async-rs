use my_async::io::{AsyncRead, AsyncWrite, ReadHalf, WriteHalf};

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

#[derive(Default)]
struct RW {
    buf: Vec<u8>,
}

impl AsyncRead for RW {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        use std::io::Read;
        let this = self.get_mut();
        let r = this.buf.as_slice().read(buf);
        Poll::Ready(r)
    }
}

impl AsyncWrite for RW {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        let this = self.get_mut();
        this.buf.extend_from_slice(buf);
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        use std::io::Write;
        let this = self.get_mut();
        let r = this.buf.as_mut_slice().flush();
        Poll::Ready(r)
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

#[test]
fn is_send_and_sync() {
    fn assert_bound<T: Send + Sync>() {}

    assert_bound::<ReadHalf<RW>>();
    assert_bound::<WriteHalf<RW>>();
}
