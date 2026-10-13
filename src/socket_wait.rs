//! Bound reads with poll instead of resetting SO_RCVTIMEO after peer closure on macOS.
use std::{io, os::fd::RawFd, time::Instant};
pub(crate) fn readable(fd: RawFd, deadline: Instant) -> io::Result<()> {
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::TimedOut, "Socket response deadline exceeded")
            })?;
        let millis = remaining
            .as_millis()
            .saturating_add(1)
            .min(i32::MAX as u128) as i32;
        let mut descriptor = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        // One live borrowed descriptor, no ownership transfer or pointer retention.
        let status = unsafe { libc::poll(&mut descriptor, 1, millis) };
        if status > 0 {
            if descriptor.revents & libc::POLLNVAL != 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Invalid socket descriptor",
                ));
            }
            return Ok(()); // Read handles data, peer EOF and pending socket errors.
        }
        if status < 0 {
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
        // Timeout/interruption returns to the absolute deadline; it never resets the budget.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        os::{fd::AsRawFd, unix::net::UnixStream},
        time::{Duration, Instant},
    };
    #[test]
    fn silent_peer_obeys_absolute_deadline() {
        let (socket, _peer) = UnixStream::pair().unwrap();
        let started = Instant::now();
        let error = readable(socket.as_raw_fd(), started + Duration::from_millis(40)).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(1));
    }
    #[test]
    fn closed_peer_is_readable_so_caller_can_process_eof() {
        let (socket, peer) = UnixStream::pair().unwrap();
        drop(peer);
        readable(socket.as_raw_fd(), Instant::now() + Duration::from_secs(1)).unwrap();
        use std::io::Read;
        assert_eq!((&socket).read(&mut [0]).unwrap(), 0);
    }
}
