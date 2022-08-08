use crate::{Interest, IoWrapper};

use std::{
    io::{self, Seek},
    path::Path,
    pin::Pin,
    task::{Context, Poll},
};

use futures_lite::io::AsyncSeek;

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
                    me.register_reactor(Interest::READABLE | Interest::WRITABLE, cx)?;
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
