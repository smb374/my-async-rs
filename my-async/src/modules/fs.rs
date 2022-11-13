//! Convenient alias for [`File`] (a.k.a [`IoWrapper<File>`]).
//!
//! The module contains predefined operations for creating [`IoWrapper<File>`].
//!
//! If you need more control on [`File`] creation, consider using [`std::fs::OpenOptions`]
//! to open or create an [`std::fs::File`] then wrap it using [`IoWrapper::from()`].

use crate::{Interest, IoWrapper};

use std::{
    io::{self, Seek},
    path::Path,
    pin::Pin,
    sync::atomic::Ordering,
    task::{Context, Poll},
};

use futures_lite::io::AsyncSeek;

/// [`File`][std::fs::File] wrapper type.
///
/// This type implements:
/// - [`create()`][File::create()] and [`open()`][File::open()]:
///   For convenient `IoWrapper<std::fs::File>` creation. See [`std::fs::File::create()`] and
///   [`std::fs::File::open()`].
///
/// For other operations, refer to [`IoWrapper`'s documentation][IoWrapper].
///
/// For more control on File open/creation, consider using [`OpenOptions`][std::fs::OpenOptions]
/// to obtain a [`std::fs::File`] and wrap it using [`IoWrapper::from()`].
pub type File = IoWrapper<std::fs::File>;

impl File {
    /// Creates a file at path `P`.
    pub fn create<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let stdfile = std::fs::File::create(path)?;
        Ok(IoWrapper::from(stdfile))
    }
    /// Opens a file at path `P`.
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
                    let current = me.token.load(Ordering::Relaxed);
                    me.register_reactor(current, Interest::READABLE | Interest::WRITABLE, cx)?;
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
