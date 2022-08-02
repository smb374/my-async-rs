// From tokio-rs/tokio/tokio/tests/fs.rs

use claim::assert_ok;
use futures_lite::{AsyncReadExt, AsyncWriteExt};
use my_async::{fs::File, multi_thread::Executor, schedulers::hybrid::HybridScheduler};

async fn path_read_write() {
    let temp = tempdir();
    let dir = temp.path();
    let path = dir.join("bar");

    let mut buf = [0u8; 4096];

    assert_ok!(assert_ok!(File::create(&path)).write(b"bytes").await);
    let n = assert_ok!(assert_ok!(File::open(&path)).read(&mut buf).await);

    assert_eq!(&buf[..n], b"bytes");
}

fn tempdir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

#[test]
fn fs() {
    let rt: Executor<HybridScheduler> = Executor::new();
    rt.block_on(path_read_write());
}
