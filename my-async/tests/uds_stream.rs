// Notes:
// 1. Remove try_* & *_ready & epollhup tests: tokio exclusive functions

#![cfg(unix)]

use claims::assert_ok;
use my_async::{
    io::{AsyncReadExt, AsyncWriteExt},
    multi_thread::{spawn, Executor},
    net::{UnixListener, UnixStream},
    schedulers::hybrid::HybridScheduler,
};

async fn accept_read_write() -> std::io::Result<()> {
    let dir = tempfile::Builder::new()
        .prefix("tokio-uds-tests")
        .tempdir()
        .unwrap();
    let sock_path = dir.path().join("connect.sock");
    let cloned = sock_path.clone();

    let listener = UnixListener::bind(&sock_path)?;

    let handle = spawn(async move {
        let mut client = assert_ok!(UnixStream::connect(&cloned));
        // Write to the client.
        assert_ok!(client.write_all(b"hello").await);
    });

    let (mut server, _) = listener.accept().await?;

    // Read from the server.
    let mut buf = [0u8; 16];
    let n = server.read(&mut buf).await?;
    assert_eq!(&buf[..n], b"hello");
    let len = server.read(&mut buf).await?;
    assert_eq!(len, 0);
    handle.join().await;
    Ok(())
}

async fn shutdown() -> std::io::Result<()> {
    let dir = tempfile::Builder::new()
        .prefix("tokio-uds-tests")
        .tempdir()
        .unwrap();
    let sock_path = dir.path().join("connect.sock");
    let cloned = sock_path.clone();

    let listener = UnixListener::bind(&sock_path)?;

    let handle = spawn(async move {
        let mut client = assert_ok!(UnixStream::connect(&cloned));
        assert_ok!(client.close().await);
    });

    let (mut server, _) = listener.accept().await?;

    // Shut down the client
    // Read from the server should return 0 to indicate the channel has been closed.
    let mut buf = [0u8; 1];
    let n = server.read(&mut buf).await?;
    assert_eq!(n, 0);
    handle.join().await;
    Ok(())
}

#[test]
fn uds_stream() {
    let rt: Executor<HybridScheduler> = Executor::new();
    rt.block_on(async {
        assert_ok!(accept_read_write().await);
        assert_ok!(shutdown().await);
    });
}
