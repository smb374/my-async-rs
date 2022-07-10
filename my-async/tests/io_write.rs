use assert_ok::assert_ok;
use my_async::{
    io::{AsyncWrite, AsyncWriteExt, Cursor},
    multi_thread::Executor,
    schedulers::hybrid::HybridScheduler,
};

use bytes::BytesMut;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

async fn write() {
    struct Wr {
        buf: BytesMut,
        cnt: usize,
    }

    impl AsyncWrite for Wr {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            assert_eq!(self.cnt, 0);
            self.buf.extend(&buf[0..4]);
            Ok(4).into()
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Ok(()).into()
        }

        fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    let mut wr = Wr {
        buf: BytesMut::with_capacity(64),
        cnt: 0,
    };

    let n = assert_ok!(wr.write(b"hello world").await);
    assert_eq!(n, 4);
    assert_eq!(wr.buf, b"hell"[..]);
}

async fn write_cursor() {
    let mut wr = Cursor::new(Vec::new());

    let n = assert_ok!(wr.write(b"hello world").await);
    assert_eq!(n, 11);
    assert_eq!(wr.get_ref().as_slice(), &b"hello world"[..]);
}

#[test]
fn io_write() {
    let rt: Executor<HybridScheduler> = Executor::new();
    rt.block_on(async {
        write().await;
        write_cursor().await;
    });
}
