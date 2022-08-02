// From tokio-rs/tokio/tokio/tests/buffered.rs

use claim::assert_ok;
use my_async::{multi_thread::Executor, net::TcpListener, schedulers::hybrid::HybridScheduler};

use std::{io::prelude::*, net::TcpStream, thread};

async fn echo_server() {
    const N: usize = 1024;

    let srv = assert_ok!(TcpListener::bind("127.0.0.1:0"));
    let addr = assert_ok!(srv.as_ref().local_addr());

    let msg = "foo bar baz";

    let t = thread::spawn(move || {
        let mut s = assert_ok!(TcpStream::connect(&addr));

        let t2 = thread::spawn(move || {
            let mut s = assert_ok!(TcpStream::connect(&addr));
            let mut b = vec![0; msg.len() * N];
            assert_ok!(s.read_exact(&mut b));
            b
        });

        let mut expected = Vec::<u8>::new();
        for _i in 0..N {
            expected.extend(msg.as_bytes());
            let res = assert_ok!(s.write(msg.as_bytes()));
            assert_eq!(res, msg.len());
        }

        (expected, t2)
    });

    let (mut a, _) = assert_ok!(srv.accept().await);
    let (mut b, _) = assert_ok!(srv.accept().await);
    let n = assert_ok!(my_async::io::copy(&mut a, &mut b).await);

    let (expected, t2) = t.join().unwrap();
    let actual = t2.join().unwrap();

    assert!(expected == actual);
    assert_eq!(n, msg.len() as u64 * 1024);
}

#[test]
fn buffered() {
    let rt: Executor<HybridScheduler> = Executor::new();
    rt.block_on(echo_server());
}
