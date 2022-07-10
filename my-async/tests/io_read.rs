use assert_ok::assert_ok;
use my_async::{
    io::{AsyncRead, AsyncReadExt},
    multi_thread::Executor,
    schedulers::hybrid::HybridScheduler,
};

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

async fn read() {
    #[derive(Default)]
    struct Rd {
        poll_cnt: usize,
    }

    impl AsyncRead for Rd {
        fn poll_read(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<io::Result<usize>> {
            let this = self.get_mut();

            assert_eq!(0, this.poll_cnt);
            this.poll_cnt += 1;
            let mut b: &[u8] = b"hello world";

            let r = io::Read::read(&mut b, buf);
            Poll::Ready(r)
        }
    }

    let mut buf = Box::new([0; 11]);
    let mut rd = Rd::default();

    let n = assert_ok!(rd.read(&mut buf[..]).await);
    assert_eq!(n, 11);
    assert_eq!(buf[..], b"hello world"[..]);
}

#[test]
fn io_read() {
    let rt: Executor<HybridScheduler> = Executor::new();
    rt.block_on(read());
}
