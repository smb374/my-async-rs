use assert_ok::assert_ok;
use my_async::{
    io::AsyncBufReadExt, multi_thread::Executor, schedulers::hybrid::HybridScheduler,
    stream::StreamExt,
};

async fn lines_inherent() {
    let rd: &[u8] = b"hello\r\nworld\n\n";
    let mut st = rd.lines();

    let b = assert_ok!(st.try_next().await).unwrap();
    assert_eq!(b, "hello");
    let b = assert_ok!(st.try_next().await).unwrap();
    assert_eq!(b, "world");
    let b = assert_ok!(st.try_next().await).unwrap();
    assert_eq!(b, "");
    assert!(assert_ok!(st.try_next().await).is_none());
}

#[test]
fn io_lines() {
    let rt: Executor<HybridScheduler> = Executor::new();
    rt.block_on(lines_inherent());
}
