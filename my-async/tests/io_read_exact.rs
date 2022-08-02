use claim::assert_ok;
use my_async::{io::AsyncReadExt, multi_thread::Executor, schedulers::hybrid::HybridScheduler};

async fn read_exact() {
    let mut buf = Box::new([0; 8]);
    let mut rd: &[u8] = b"hello world";

    assert_ok!(rd.read_exact(&mut buf[..]).await);
    assert_eq!(buf[..], b"hello wo"[..]);
}

#[test]
fn io_read_exact() {
    let rt: Executor<HybridScheduler> = Executor::new();
    rt.block_on(read_exact());
}
