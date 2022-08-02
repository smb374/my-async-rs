use claim::assert_ok;
use my_async::{io::AsyncReadExt, multi_thread::Executor, schedulers::hybrid::HybridScheduler};

async fn read_to_string() {
    let mut buf = String::new();
    let mut rd: &[u8] = b"hello world";

    let n = assert_ok!(rd.read_to_string(&mut buf).await);
    assert_eq!(n, 11);
    assert_eq!(buf[..], "hello world"[..]);
}

#[test]
fn io_read_to_string() {
    let rt: Executor<HybridScheduler> = Executor::new();
    rt.block_on(read_to_string());
}
