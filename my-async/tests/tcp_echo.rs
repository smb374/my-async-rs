use claims::assert_ok;
use my_async::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    multi_thread::{spawn, Executor},
    net::{TcpListener, TcpStream},
    schedulers::hybrid::HybridScheduler,
};

async fn echo_server() {
    const ITER: usize = 1024;

    let (tx, rx) = flume::unbounded();

    let srv = assert_ok!(TcpListener::bind("127.0.0.1:0"));
    let addr = assert_ok!(srv.as_ref().local_addr());

    let msg = "foo bar baz";
    spawn(async move {
        let mut stream = assert_ok!(TcpStream::connect(addr));

        for _ in 0..ITER {
            // write
            assert_ok!(stream.write_all(msg.as_bytes()).await);

            // read
            let mut buf = [0; 11];
            assert_ok!(stream.read_exact(&mut buf).await);
            assert_eq!(&buf[..], msg.as_bytes());
        }

        assert_ok!(tx.send(()));
    });

    let (stream, _) = assert_ok!(srv.accept().await);
    let (mut rd, mut wr) = io::split(stream);

    let n = assert_ok!(io::copy(&mut rd, &mut wr).await);
    assert_eq!(n, (ITER * msg.len()) as u64);

    assert_ok!(rx.recv());
}

#[test]
fn tcp_echo() {
    let rt: Executor<HybridScheduler> = Executor::new();
    rt.block_on(echo_server());
}
