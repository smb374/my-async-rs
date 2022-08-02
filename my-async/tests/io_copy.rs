use claim::assert_ok;
use my_async::io::{self, split, AsyncRead, AsyncWriteExt, Cursor};
use my_async::multi_thread::Executor;
use my_async::schedulers::hybrid::HybridScheduler;

use std::io::Read;
use std::pin::Pin;
use std::task::{Context, Poll};

async fn copy() {
    struct Rd(bool);

    impl AsyncRead for Rd {
        fn poll_read(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<io::Result<usize>> {
            if self.0 {
                // buf.copy_from_slice(b"hello world");
                let x: Vec<u8> = b"hello world".to_vec();
                assert_ok!(Read::read(&mut x.as_slice(), buf));
                self.0 = false;
                Poll::Ready(Ok(11))
            } else {
                Poll::Ready(Ok(0))
            }
        }
    }

    let mut rd = Rd(true);
    let mut wr = Vec::new();

    let n = assert_ok!(io::copy(&mut rd, &mut wr).await);
    assert_eq!(n, 11);
    assert_eq!(wr, b"hello world");
}

async fn proxy() {
    let stream = Cursor::new(vec![0; 1024]);
    let (mut rd, mut wd) = split(stream);

    // write start bytes
    assert_ok!(wd.write_all(&[0x42; 512]).await);
    assert_ok!(wd.flush().await);

    let n = assert_ok!(io::copy(&mut rd, &mut wd).await);

    assert_eq!(n, 512);
}

#[test]
fn io_copy() {
    let rt: Executor<HybridScheduler> = Executor::new();
    rt.block_on(async {
        copy().await;
        proxy().await;
    });
}
