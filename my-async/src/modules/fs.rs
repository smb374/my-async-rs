use crate::{impl_common_write, IoWrapper};

use std::{
    io::{self, Seek, Write},
    path::Path,
    pin::Pin,
    sync::atomic::Ordering,
    task::{Context, Poll},
};

use futures_lite::io::{AsyncSeek, AsyncWrite};
use polling::Event;

pub type File = IoWrapper<std::fs::File>;

impl File {
    pub fn create<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let stdfile = std::fs::File::create(path)?;
        Ok(IoWrapper::from(stdfile))
    }
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let stdfile = std::fs::File::open(path)?;
        Ok(IoWrapper::from(stdfile))
    }
}

impl AsyncWrite for File {
    impl_common_write!();
    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        // No extra close is needed
        // since Rust will close on drop.
        // We simply need to flush it.
        self.poll_flush(cx)
    }
}

impl AsyncSeek for File {
    fn poll_seek(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        pos: io::SeekFrom,
    ) -> Poll<io::Result<u64>> {
        let me = self.get_mut();
        match me.inner.seek(pos) {
            Err(e) => match e.kind() {
                // Pin self again and retry.
                io::ErrorKind::Interrupted => Pin::new(me).poll_seek(cx, pos),
                // Register self to reactor and wait.
                io::ErrorKind::WouldBlock => {
                    let key = me.key.load(Ordering::Relaxed);
                    let interest = Event {
                        key,
                        readable: true,
                        writable: true,
                    };
                    me.register_reactor(interest, cx)?;
                    Poll::Pending
                }
                // Other errors are returned directly.
                _ => Poll::Ready(Err(e)),
            },
            // Success, return result.
            Ok(i) => Poll::Ready(Ok(i)),
        }
    }
}
