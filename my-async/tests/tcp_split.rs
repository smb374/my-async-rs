use std::io::Result;
use std::io::{Read, Write};
use std::{net, thread};

use assert_ok::assert_ok;
use my_async::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    multi_thread::Executor,
    net::TcpStream,
    schedulers::hybrid::HybridScheduler,
};

async fn split() -> Result<()> {
    const MSG: &[u8] = b"split";

    let listener = net::TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;

    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.write_all(MSG).unwrap();

        let mut read_buf = [0u8; 32];
        let read_len = stream.read(&mut read_buf).unwrap();
        assert_eq!(&read_buf[..read_len], MSG);
    });

    let stream = TcpStream::connect(&addr)?;
    let (mut read_half, mut write_half) = io::split(stream);

    let mut read_buf = [0u8; 32];
    let read_len = read_half.read(&mut read_buf[..]).await?;
    assert_eq!(&read_buf[..read_len], MSG);

    write_half.write(MSG).await?;
    handle.join().unwrap();
    Ok(())
}

#[test]
fn tcp_split() {
    let rt: Executor<HybridScheduler> = Executor::new();
    rt.block_on(async {
        assert_ok!(split().await);
    });
}
