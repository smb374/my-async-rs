//! Reexports [`std::io`] and [`futures_lite::io`].

pub use std::io::{Error, ErrorKind, Result, SeekFrom};

pub use futures_lite::io::*;
