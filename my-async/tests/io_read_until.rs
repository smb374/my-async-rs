use assert_ok::assert_ok;
use my_async::{
    io::{AsyncBufReadExt, BufReader, Cursor},
    multi_thread::Executor,
    schedulers::hybrid::HybridScheduler,
};

async fn read_until() {
    let mut buf = vec![];
    let mut rd: &[u8] = b"hello world";

    let n = assert_ok!(rd.read_until(b' ', &mut buf).await);
    assert_eq!(n, 6);
    assert_eq!(buf, b"hello ");
    buf.clear();
    let n = assert_ok!(rd.read_until(b' ', &mut buf).await);
    assert_eq!(n, 5);
    assert_eq!(buf, b"world");
    buf.clear();
    let n = assert_ok!(rd.read_until(b' ', &mut buf).await);
    assert_eq!(n, 0);
    assert_eq!(buf, []);
}

async fn read_until_not_all_ready() {
    let cursor = Cursor::new(b"Hello World#Fizz\xffBuzz#1#2");

    let mut read = BufReader::new(cursor);

    let mut chunk = b"We say ".to_vec();
    let bytes = read.read_until(b'#', &mut chunk).await.unwrap();
    assert_eq!(bytes, b"Hello World#".len());
    assert_eq!(chunk, b"We say Hello World#");

    chunk = b"I solve ".to_vec();
    let bytes = read.read_until(b'#', &mut chunk).await.unwrap();
    assert_eq!(bytes, b"Fizz\xffBuzz\n".len());
    assert_eq!(chunk, b"I solve Fizz\xffBuzz#");

    chunk.clear();
    let bytes = read.read_until(b'#', &mut chunk).await.unwrap();
    assert_eq!(bytes, 2);
    assert_eq!(chunk, b"1#");

    chunk.clear();
    let bytes = read.read_until(b'#', &mut chunk).await.unwrap();
    assert_eq!(bytes, 1);
    assert_eq!(chunk, b"2");
}

#[test]
fn io_read_until() {
    let rt: Executor<HybridScheduler> = Executor::new();
    rt.block_on(async {
        read_until().await;
        read_until_not_all_ready().await;
    });
}
