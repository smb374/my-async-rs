use claim::assert_ok;
use my_async::{io::AsyncReadExt, multi_thread::Executor, schedulers::hybrid::HybridScheduler};

async fn take() {
    let mut buf = [0; 6];
    let rd: &[u8] = b"hello world";

    let mut rd = rd.take(4);
    let n = assert_ok!(rd.read(&mut buf).await);
    assert_eq!(n, 4);
    assert_eq!(&buf, &b"hell\0\0"[..]);
}

#[test]
fn io_take() {
    let rt: Executor<HybridScheduler> = Executor::new();
    rt.block_on(take());
}
