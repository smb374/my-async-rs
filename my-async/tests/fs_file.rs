// From tokio-rs/tokio/tokio/tests/fs_file.rs

use claim::assert_ok;
use my_async::{
    fs::File,
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, SeekFrom},
    multi_thread::Executor,
    schedulers::hybrid::HybridScheduler,
};
use std::io::prelude::*;
use tempfile::NamedTempFile;

const HELLO: &[u8] = b"hello world...";

async fn basic_read() {
    let mut tempfile = tempfile();
    tempfile.write_all(HELLO).unwrap();

    let mut file = assert_ok!(File::open(tempfile.path()));

    let mut buf = [0; 1024];
    let n = assert_ok!(file.read(&mut buf).await);

    assert_eq!(n, HELLO.len());
    assert_eq!(&buf[..n], HELLO);
}

async fn basic_write() {
    let tempfile = tempfile();

    let mut file = assert_ok!(File::create(tempfile.path()));

    assert_ok!(file.write_all(HELLO).await);
    assert_ok!(file.flush().await);

    let file = assert_ok!(std::fs::read(tempfile.path()));
    assert_eq!(file, HELLO);
}

async fn basic_write_and_shutdown() {
    let tempfile = tempfile();

    let mut file = assert_ok!(File::create(tempfile.path()));

    assert_ok!(file.write_all(HELLO).await);
    drop(file);

    let file2 = assert_ok!(std::fs::read(tempfile.path()));
    assert_eq!(file2, HELLO);
}

async fn rewind_seek_position() {
    let tempfile = tempfile();

    let mut file = assert_ok!(File::create(tempfile.path()));

    assert_ok!(file.seek(SeekFrom::Current(10)).await);

    assert_ok!(file.seek(SeekFrom::Start(0)).await);

    assert_eq!(file.as_ref().stream_position().unwrap(), 0);
}

fn tempfile() -> NamedTempFile {
    NamedTempFile::new().unwrap()
}

#[test]
fn fs_file() {
    let rt: Executor<HybridScheduler> = Executor::new();
    rt.block_on(async move {
        basic_read().await;
        println!("basic_read passed");
        basic_write().await;
        println!("basic_write passed");
        basic_write_and_shutdown().await;
        println!("basic_write_and_shutdown passed");
        rewind_seek_position().await;
        println!("rewind_seek_position passed");
    });
}
