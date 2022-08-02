use claim::assert_ok;
use my_async::{
    io::{AsyncWrite, AsyncWriteExt},
    multi_thread::Executor,
    schedulers::hybrid::HybridScheduler,
};

use bytes::BytesMut;
use std::cmp;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

async fn write_all() {
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
            let n = cmp::min(4, buf.len());
            let buf = &buf[0..n];

            self.cnt += 1;
            self.buf.extend(buf);
            Ok(buf.len()).into()
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

    assert_ok!(wr.write_all(b"hello world").await);
    assert_eq!(wr.buf, b"hello world"[..]);
    assert_eq!(wr.cnt, 3);
}

#[test]
fn io_write_all() {
    let rt: Executor<HybridScheduler> = Executor::new();
    rt.block_on(write_all());
}
