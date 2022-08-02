use claim::assert_ok;
use my_async::{
    io::{AsyncBufReadExt, BufReader, Cursor},
    multi_thread::Executor,
    schedulers::hybrid::HybridScheduler,
};
use std::io::ErrorKind;

async fn read_line() {
    let mut buf = String::new();
    let mut rd = Cursor::new(b"hello\nworld\n\n");

    let n = assert_ok!(rd.read_line(&mut buf).await);
    assert_eq!(n, 6);
    assert_eq!(buf, "hello\n");
    buf.clear();
    let n = assert_ok!(rd.read_line(&mut buf).await);
    assert_eq!(n, 6);
    assert_eq!(buf, "world\n");
    buf.clear();
    let n = assert_ok!(rd.read_line(&mut buf).await);
    assert_eq!(n, 1);
    assert_eq!(buf, "\n");
    buf.clear();
    let n = assert_ok!(rd.read_line(&mut buf).await);
    assert_eq!(n, 0);
    assert_eq!(buf, "");
}

async fn read_line_not_all_ready() {
    let mut read = BufReader::new(Cursor::new(b"Hello World\nFizzBuzz\n1\n2"));

    let mut line = "".to_string();
    let bytes = assert_ok!(read.read_line(&mut line).await);
    assert_eq!(bytes, "Hello World\n".len());
    assert_eq!(line.as_str(), "Hello World\n");

    line.clear();
    let bytes = assert_ok!(read.read_line(&mut line).await);
    assert_eq!(bytes, "FizzBuzz\n".len());
    assert_eq!(line.as_str(), "FizzBuzz\n");

    line.clear();
    let bytes = assert_ok!(read.read_line(&mut line).await);
    assert_eq!(bytes, 2);
    assert_eq!(line.as_str(), "1\n");

    line.clear();
    let bytes = assert_ok!(read.read_line(&mut line).await);
    assert_eq!(bytes, 1);
    assert_eq!(line.as_str(), "2");
}

async fn read_line_invalid_utf8() {
    let mut read = BufReader::new(Cursor::new(b"Hello Wor\xffld.\n"));

    let mut line = "Foo".to_string();
    let err = read.read_line(&mut line).await.expect_err("Should fail");
    assert_eq!(err.kind(), ErrorKind::InvalidData);
    assert_eq!(err.to_string(), "stream did not contain valid UTF-8");
    assert_eq!(line.as_str(), "Foo");
}

#[test]
fn io_read_line() {
    let rt: Executor<HybridScheduler> = Executor::new();
    rt.block_on(async {
        println!("read line");
        read_line().await;
        println!("read line not all ready");
        read_line_not_all_ready().await;
        println!("read line invalid utf8");
        read_line_invalid_utf8().await;
    });
}
