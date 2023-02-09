use claims::assert_ok;
use my_async::fs::File;
use my_async::io::{AsyncBufReadExt, BufReader};
use my_async::multi_thread::Executor;
use my_async::schedulers::hybrid::HybridScheduler;
use tempfile::NamedTempFile;

async fn fill_buf_file() {
    let file = NamedTempFile::new().unwrap();

    assert_ok!(std::fs::write(file.path(), b"hello"));

    let file = assert_ok!(File::open(file.path()));
    let mut file = BufReader::new(file);

    let mut contents = Vec::new();

    loop {
        let consumed = {
            let buffer = assert_ok!(file.fill_buf().await);
            if buffer.is_empty() {
                break;
            }
            contents.extend_from_slice(buffer);
            buffer.len()
        };

        file.consume(consumed);
    }

    assert_eq!(contents, b"hello");
}

#[test]
fn io_fill_buf() {
    let rt: Executor<HybridScheduler> = Executor::new();
    rt.block_on(fill_buf_file());
}
