use std::io;

use rustix::{
    fd::AsFd,
    fs::{fcntl_getfl, fcntl_setfl, OFlags},
};

pub fn set_nonblocking<Fd: AsFd>(fd: &Fd) -> io::Result<()> {
    let mut current_flags =
        fcntl_getfl(fd).map_err(|e| io::Error::from_raw_os_error(e.raw_os_error()))?;
    if !current_flags.contains(OFlags::NONBLOCK) {
        current_flags.set(OFlags::NONBLOCK, true);
        fcntl_setfl(fd, current_flags)
            .map_err(|e| io::Error::from_raw_os_error(e.raw_os_error()))?;
    }
    Ok(())
}
