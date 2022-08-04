use claim::assert_ok;
use futures_lite::future::zip;
use my_async::{
    multi_thread::{spawn, Executor},
    net::{TcpListener, TcpStream},
    schedulers::hybrid::HybridScheduler,
};

async fn connect_v4() {
    let srv = assert_ok!(TcpListener::bind("127.0.0.1:0"));
    let addr = assert_ok!(srv.as_ref().local_addr());
    assert!(addr.is_ipv4());

    let (tx, rx) = flume::unbounded();

    let handle = spawn(async move {
        let (socket, addr) = assert_ok!(srv.accept().await);
        assert_eq!(addr, assert_ok!(socket.as_ref().peer_addr()));
        assert_ok!(tx.send(socket));
    });

    let mine = assert_ok!(TcpStream::connect(&addr));
    let theirs = assert_ok!(rx.recv());

    assert_eq!(
        assert_ok!(mine.as_ref().local_addr()),
        assert_ok!(theirs.as_ref().peer_addr())
    );
    assert_eq!(
        assert_ok!(theirs.as_ref().local_addr()),
        assert_ok!(mine.as_ref().peer_addr())
    );
    handle.join().await;
}

#[allow(dead_code)]
async fn connect_v6() {
    let srv = assert_ok!(TcpListener::bind("[::1]:0"));
    let addr = assert_ok!(srv.as_ref().local_addr());
    assert!(addr.is_ipv6());

    let (tx, rx) = flume::unbounded();

    let handle = spawn(async move {
        let (socket, addr) = assert_ok!(srv.accept().await);
        assert_eq!(addr, assert_ok!(socket.as_ref().peer_addr()));
        assert_ok!(tx.send(socket));
    });

    let mine = assert_ok!(TcpStream::connect(&addr));
    let theirs = assert_ok!(rx.recv());

    assert_eq!(
        assert_ok!(mine.as_ref().local_addr()),
        assert_ok!(theirs.as_ref().peer_addr())
    );
    assert_eq!(
        assert_ok!(theirs.as_ref().local_addr()),
        assert_ok!(mine.as_ref().peer_addr())
    );
    handle.join().await;
}

async fn connect_addr_ip_string() {
    let srv = assert_ok!(TcpListener::bind("127.0.0.1:0"));
    let addr = assert_ok!(srv.as_ref().local_addr());
    let addr = format!("127.0.0.1:{}", addr.port());

    let server = async {
        assert_ok!(srv.accept().await);
    };

    let client = async {
        assert_ok!(TcpStream::connect(addr));
    };

    zip(server, client).await;
}

async fn connect_addr_ip_str_slice() {
    let srv = assert_ok!(TcpListener::bind("127.0.0.1:0"));
    let addr = assert_ok!(srv.as_ref().local_addr());
    let addr = format!("127.0.0.1:{}", addr.port());

    let server = async {
        assert_ok!(srv.accept().await);
    };

    let client = async {
        assert_ok!(TcpStream::connect(&addr[..]));
    };

    zip(server, client).await;
}

async fn connect_addr_host_string() {
    let srv = assert_ok!(TcpListener::bind("127.0.0.1:0"));
    let addr = assert_ok!(srv.as_ref().local_addr());
    let addr = format!("localhost:{}", addr.port());

    let server = async {
        assert_ok!(srv.accept().await);
    };

    let client = async {
        assert_ok!(TcpStream::connect(addr));
    };

    zip(server, client).await;
}

async fn connect_addr_ip_port_tuple() {
    let srv = assert_ok!(TcpListener::bind("127.0.0.1:0"));
    let addr = assert_ok!(srv.as_ref().local_addr());
    let addr = (addr.ip(), addr.port());

    let server = async {
        assert_ok!(srv.accept().await);
    };

    let client = async {
        assert_ok!(TcpStream::connect(&addr));
    };

    zip(server, client).await;
}

async fn connect_addr_ip_str_port_tuple() {
    let srv = assert_ok!(TcpListener::bind("127.0.0.1:0"));
    let addr = assert_ok!(srv.as_ref().local_addr());
    let addr = ("127.0.0.1", addr.port());

    let server = async {
        assert_ok!(srv.accept().await);
    };

    let client = async {
        assert_ok!(TcpStream::connect(&addr));
    };

    zip(server, client).await;
}

async fn connect_addr_host_str_port_tuple() {
    let srv = assert_ok!(TcpListener::bind("127.0.0.1:0"));
    let addr = assert_ok!(srv.as_ref().local_addr());
    let addr = ("localhost", addr.port());

    let server = async {
        assert_ok!(srv.accept().await);
    };

    let client = async {
        assert_ok!(TcpStream::connect(&addr));
    };

    zip(server, client).await;
}

#[test]
fn tcp_connect() {
    let rt: Executor<HybridScheduler> = Executor::new();
    rt.block_on(async {
        connect_v4().await;
        // connect_v6().await;
        connect_addr_ip_string().await;
        connect_addr_ip_str_slice().await;
        connect_addr_host_string().await;
        connect_addr_ip_port_tuple().await;
        connect_addr_ip_str_port_tuple().await;
        connect_addr_host_str_port_tuple().await;
    });
}
