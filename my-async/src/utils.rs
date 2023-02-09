use std::{io, os::fd::AsFd};

use rustix::fs::{fcntl_getfl, fcntl_setfl, OFlags};

/// A convenient function that sets the file descriptor with `O_NONBLOCK`.
pub fn set_nonblocking<Fd: AsFd>(fd: &Fd) -> io::Result<()> {
    let mut current_flags = fcntl_getfl(fd)?;
    if !current_flags.contains(OFlags::NONBLOCK) {
        current_flags.set(OFlags::NONBLOCK, true);
        fcntl_setfl(fd, current_flags)?;
    }
    Ok(())
}
