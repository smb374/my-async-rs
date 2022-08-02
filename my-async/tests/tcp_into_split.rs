use std::io::Result;

use claim::assert_ok;
use futures_lite::future::try_zip;
use my_async::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    multi_thread::Executor,
    net::{TcpListener, TcpStream},
    schedulers::hybrid::HybridScheduler,
};

async fn split() -> Result<()> {
    const MSG: &[u8] = b"split";
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.as_ref().local_addr()?;

    let a = async move {
        let (mut stream, _) = listener.accept().await?;
        let len = stream.write(MSG).await?;
        assert_eq!(len, MSG.len());

        let mut read_buf = vec![0u8; 32];
        let read_len = stream.read(&mut read_buf).await?;
        assert_eq!(&read_buf[..read_len], MSG);
        Result::Ok(())
    };
    let b = async move {
        let stream = TcpStream::connect(addr)?;
        let (mut read_half, mut write_half) = io::split(stream);

        let mut read_buf = vec![0u8; 32];
        let read_len = read_half.read(&mut read_buf[..]).await?;
        assert_eq!(&read_buf[..read_len], MSG);
        let len = write_half.write(MSG).await?;
        assert_eq!(len, MSG.len());

        Result::Ok(())
    };
    try_zip(a, b).await?;
    Ok(())
}

#[test]
fn tcp_into_split() {
    let rt: Executor<HybridScheduler> = Executor::new();
    rt.block_on(async {
        assert_ok!(split().await);
    });
}
