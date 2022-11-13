//! Reexports [`std::io`] and [`futures_lite::io`](https://docs.rs/futures-lite/1.12.0/futures_lite/io/index.html).

pub use std::io::{Error, ErrorKind, Result, SeekFrom};

pub use futures_lite::io::*;
