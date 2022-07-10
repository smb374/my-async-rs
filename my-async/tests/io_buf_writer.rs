// From rust-lang/futures-rs/futures/tests/io_buf_writer.rs

use futures_lite::io::{AsyncSeekExt, AsyncWriteExt, BufWriter, Cursor, SeekFrom};
use my_async::{multi_thread::Executor, schedulers::hybrid::HybridScheduler};

async fn buf_writer() {
    let mut writer = BufWriter::with_capacity(2, Vec::new());

    writer.write(&[0, 1]).await.unwrap();
    assert_eq!(writer.buffer(), []);
    assert_eq!(*writer.get_ref(), [0, 1]);

    writer.write(&[2]).await.unwrap();
    assert_eq!(writer.buffer(), [2]);
    assert_eq!(*writer.get_ref(), [0, 1]);

    writer.write(&[3]).await.unwrap();
    assert_eq!(writer.buffer(), [2, 3]);
    assert_eq!(*writer.get_ref(), [0, 1]);

    writer.flush().await.unwrap();
    assert_eq!(writer.buffer(), []);
    assert_eq!(*writer.get_ref(), [0, 1, 2, 3]);

    writer.write(&[4]).await.unwrap();
    writer.write(&[5]).await.unwrap();
    assert_eq!(writer.buffer(), [4, 5]);
    assert_eq!(*writer.get_ref(), [0, 1, 2, 3]);

    writer.write(&[6]).await.unwrap();
    assert_eq!(writer.buffer(), [6]);
    assert_eq!(*writer.get_ref(), [0, 1, 2, 3, 4, 5]);

    writer.write(&[7, 8]).await.unwrap();
    assert_eq!(writer.buffer(), []);
    assert_eq!(*writer.get_ref(), [0, 1, 2, 3, 4, 5, 6, 7, 8]);

    writer.write(&[9, 10, 11]).await.unwrap();
    assert_eq!(writer.buffer(), []);
    assert_eq!(*writer.get_ref(), [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);

    writer.flush().await.unwrap();
    assert_eq!(writer.buffer(), []);
    assert_eq!(*writer.get_ref(), [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
}

async fn buf_writer_inner_flushes() {
    let mut w = BufWriter::with_capacity(3, Vec::new());
    w.write(&[0, 1]).await.unwrap();
    assert_eq!(*w.get_ref(), []);
    w.flush().await.unwrap();
    let w = w.into_inner();
    assert_eq!(w, [0, 1]);
}

async fn buf_writer_seek() {
    // FIXME: when https://github.com/rust-lang/futures-rs/issues/1510 fixed,
    // use `Vec::new` instead of `vec![0; 8]`.
    let mut w = BufWriter::with_capacity(3, Cursor::new(vec![0; 8]));
    w.write_all(&[0, 1, 2, 3, 4, 5]).await.unwrap();
    w.write_all(&[6, 7]).await.unwrap();
    assert_eq!(w.seek(SeekFrom::Current(0)).await.ok(), Some(8));
    assert_eq!(&w.get_ref().get_ref()[..], &[0, 1, 2, 3, 4, 5, 6, 7][..]);
    assert_eq!((w.seek(SeekFrom::Start(2))).await.ok(), Some(2));
    w.write_all(&[8, 9]).await.unwrap();
    w.flush().await.unwrap();
    assert_eq!(&w.into_inner().into_inner()[..], &[0, 1, 8, 9, 4, 5, 6, 7]);
}

#[test]
fn io_buf_writer() {
    let rt: Executor<HybridScheduler> = Executor::new();
    rt.block_on(async {
        buf_writer().await;
        buf_writer_inner_flushes().await;
        buf_writer_seek().await;
    })
}
