use assert_ok::assert_ok;
use my_async::{io::AsyncReadExt, multi_thread::Executor, schedulers::hybrid::HybridScheduler};

async fn read_to_end() {
    let mut buf = vec![];
    let mut rd: &[u8] = b"hello world";

    let n = assert_ok!(rd.read_to_end(&mut buf).await);
    assert_eq!(n, 11);
    assert_eq!(buf[..], b"hello world"[..]);
}

#[test]
fn io_read_to_end() {
    let rt: Executor<HybridScheduler> = Executor::new();
    rt.block_on(read_to_end());
}
