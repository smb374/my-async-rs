use claim::assert_ok;
use my_async::{io::AsyncReadExt, multi_thread::Executor, schedulers::hybrid::HybridScheduler};

async fn chain() {
    let mut buf = Vec::new();
    let rd1: &[u8] = b"hello ";
    let rd2: &[u8] = b"world";

    let mut rd = rd1.chain(rd2);
    assert_ok!(rd.read_to_end(&mut buf).await);
    assert_eq!(buf, b"hello world");
}

#[test]
fn io_chain() {
    let rt: Executor<HybridScheduler> = Executor::new();
    rt.block_on(chain());
}
