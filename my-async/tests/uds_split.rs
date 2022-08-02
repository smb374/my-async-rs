#![cfg(unix)]

use claim::assert_ok;
use my_async::{
    io::{self, AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf},
    multi_thread::Executor,
    net::UnixStream,
    schedulers::hybrid::HybridScheduler,
};

/// Checks that `UnixStream` can be split into a read half and a write half using
/// `UnixStream::split` and `UnixStream::split_mut`.
///
/// Verifies that the implementation of `AsyncWrite::poll_shutdown` shutdowns the stream for
/// writing by reading to the end of stream on the other side of the connection.
async fn split() -> std::io::Result<()> {
    let (a, b) = UnixStream::pair()?;

    let (mut a_read, mut a_write) = io::split(a);
    let (mut b_read, mut b_write) = io::split(b);

    let (a_response, b_response) = futures_lite::future::try_zip(
        send_recv_all(&mut a_read, &mut a_write, b"A"),
        send_recv_all(&mut b_read, &mut b_write, b"B"),
    )
    .await?;

    assert_eq!(a_response, b"B");
    assert_eq!(b_response, b"A");

    Ok(())
}

async fn send_recv_all(
    read: &mut ReadHalf<UnixStream>,
    write: &mut WriteHalf<UnixStream>,
    input: &[u8],
) -> std::io::Result<Vec<u8>> {
    write.write_all(input).await?;
    write.close().await?;

    let mut output = [0u8; 16];
    let n = read.read(&mut output).await?;
    Ok((&output[..n]).to_vec())
}

#[test]
fn uds_split() {
    let rt: Executor<HybridScheduler> = Executor::new();
    rt.block_on(async {
        assert_ok!(split().await);
    });
}
