// Notes:
// 1. Remove split_* tests: Due to the limitation of Future (poll() requires Pin<&mut self>)
// 2. Remove try_* & poll_ready tests: tokio exclusive functions

#![cfg(unix)]

use claim::assert_ok;
use my_async::{
    multi_thread::{spawn, Executor},
    net::UnixDatagram,
    schedulers::hybrid::HybridScheduler,
};

use std::io;

async fn echo_server(socket: UnixDatagram) -> io::Result<()> {
    let mut recv_buf = vec![0u8; 1024];
    loop {
        let (len, peer_addr) = socket.recv_from(&mut recv_buf[..]).await?;
        if let Some(path) = peer_addr.as_pathname() {
            socket.send_to(&recv_buf[..len], path).await?;
        }
    }
}

async fn echo() -> io::Result<()> {
    let dir = tempfile::tempdir().unwrap();
    let server_path = dir.path().join("server.sock");
    let client_path = dir.path().join("client.sock");

    let server_socket = UnixDatagram::bind(server_path.clone())?;

    spawn(async move {
        if let Err(e) = echo_server(server_socket).await {
            eprintln!("Error in echo server: {}", e);
        }
    });

    {
        let socket = UnixDatagram::bind(&client_path).unwrap();
        socket.connect(server_path).await?;
        socket.send(b"ECHO").await?;
        let mut recv_buf = [0u8; 16];
        let len = socket.recv(&mut recv_buf[..]).await?;
        assert_eq!(&recv_buf[..len], b"ECHO");
    }

    Ok(())
}

async fn echo_from() -> io::Result<()> {
    let dir = tempfile::tempdir().unwrap();
    let server_path = dir.path().join("server.sock");
    let client_path = dir.path().join("client.sock");

    let server_socket = UnixDatagram::bind(server_path.clone())?;

    spawn(async move {
        if let Err(e) = echo_server(server_socket).await {
            eprintln!("Error in echo server: {}", e);
        }
    });

    {
        let socket = UnixDatagram::bind(&client_path).unwrap();
        socket.connect(&server_path).await?;
        socket.send(b"ECHO").await?;
        let mut recv_buf = [0u8; 16];
        let (len, addr) = socket.recv_from(&mut recv_buf[..]).await?;
        assert_eq!(&recv_buf[..len], b"ECHO");
        assert_eq!(addr.as_pathname(), Some(server_path.as_path()));
    }

    Ok(())
}

#[test]
fn uds_datagram() {
    let rt: Executor<HybridScheduler> = Executor::new();
    rt.block_on(async {
        assert_ok!(echo().await);
        assert_ok!(echo_from().await);
    });
}
