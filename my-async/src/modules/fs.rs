use crate::IoWrapper;

use std::{
    io::{self, Seek},
    path::Path,
    pin::Pin,
    sync::atomic::Ordering,
    task::{Context, Poll},
};

use futures_lite::io::AsyncSeek;
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

impl AsyncSeek for File {
    fn poll_seek(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        pos: io::SeekFrom,
    ) -> Poll<io::Result<u64>> {
        let key = self.key.load(Ordering::Relaxed);
        self.poll_pinned(cx, Event::all(key), |x| x.inner.seek(pos))
    }
}
