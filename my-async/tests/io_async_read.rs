// From tokio-rs/tokio/tokio/tests/io_async_read.rs

use my_async::io::AsyncRead;

#[test]
fn assert_obj_safe() {
    fn _assert<T>() {}
    _assert::<Box<dyn AsyncRead>>();
}
