// Notes:
// 1. Remove split_* tests: Due to the limitation of Future (poll() requires Pin<&mut self>)
// 2. Remove try_* & poll_ready tests: tokio exclusive functions

use claims::assert_ok;
use my_async::{multi_thread::Executor, net::UdpSocket, schedulers::hybrid::HybridScheduler};

const MSG: &[u8] = b"hello";

async fn send_recv() -> std::io::Result<()> {
    let sender = UdpSocket::bind("127.0.0.1:0")?;
    let receiver = UdpSocket::bind("127.0.0.1:0")?;

    sender.connect(receiver.inner().local_addr()?).await?;
    receiver.connect(sender.inner().local_addr()?).await?;

    sender.send(MSG).await?;

    let mut recv_buf = [0u8; 32];
    let len = receiver.recv(&mut recv_buf[..]).await?;

    assert_eq!(&recv_buf[..len], MSG);
    Ok(())
}

async fn send_to_recv_from() -> std::io::Result<()> {
    let sender = UdpSocket::bind("127.0.0.1:0")?;
    let receiver = UdpSocket::bind("127.0.0.1:0")?;

    let receiver_addr = receiver.inner().local_addr()?;
    sender.send_to(MSG, &receiver_addr).await?;

    let mut recv_buf = [0u8; 32];
    let (len, addr) = receiver.recv_from(&mut recv_buf[..]).await?;

    assert_eq!(&recv_buf[..len], MSG);
    assert_eq!(addr, sender.inner().local_addr()?);
    Ok(())
}

async fn send_to_peek_from() -> std::io::Result<()> {
    let sender = UdpSocket::bind("127.0.0.1:0")?;
    let receiver = UdpSocket::bind("127.0.0.1:0")?;

    let receiver_addr = receiver.inner().local_addr()?;
    sender.send_to(MSG, receiver_addr).await?;

    // peek
    let mut recv_buf = [0u8; 32];
    let (n, addr) = receiver.peek_from(&mut recv_buf).await?;
    assert_eq!(&recv_buf[..n], MSG);
    assert_eq!(addr, sender.inner().local_addr()?);

    // peek
    let mut recv_buf = [0u8; 32];
    let (n, addr) = receiver.peek_from(&mut recv_buf).await?;
    assert_eq!(&recv_buf[..n], MSG);
    assert_eq!(addr, sender.inner().local_addr()?);

    let mut recv_buf = [0u8; 32];
    let (n, addr) = receiver.recv_from(&mut recv_buf).await?;
    assert_eq!(&recv_buf[..n], MSG);
    assert_eq!(addr, sender.inner().local_addr()?);

    Ok(())
}

#[test]
fn udp() {
    let rt: Executor<HybridScheduler> = Executor::new();
    rt.block_on(async {
        assert_ok!(send_recv().await);
        assert_ok!(send_to_peek_from().await);
        assert_ok!(send_to_recv_from().await);
    });
}
