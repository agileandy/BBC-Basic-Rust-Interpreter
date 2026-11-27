//! Read one key from stdin for GET, GET$, INKEY and INKEY$.
//!
//! On a terminal, the read uses raw mode, so that the key comes with no
//! Return key and no echo. With piped input, the read takes the next byte.

use std::io::{self, IsTerminal, Read, StdinLock, Write};
use std::time::Duration;

/// The byte that Ctrl-C gives in raw mode.
const CTRL_C: u8 = 0x03;

/// Read one key from stdin. Give None at the end of input or on a read error.
///
/// The read goes through the shared stdin buffer, so it takes the next byte
/// after the lines that the REPL and INPUT already read.
pub(crate) fn read_key() -> Option<u8> {
    read_from_stdin(read_byte)
}

/// Read one key from stdin, but wait for a maximum of `limit`. Give None if
/// no key comes in that time, at the end of input or on a read error.
///
/// At the end of input the result comes at once, with no wait.
pub(crate) fn read_key_within(limit: Duration) -> Option<u8> {
    read_from_stdin(|input| timed::read_byte_within(input, limit))
}

/// Lock stdin, put a terminal in raw mode, and read one byte with `read`.
fn read_from_stdin(read: impl FnOnce(&mut StdinLock<'static>) -> Option<u8>) -> Option<u8> {
    // Write the pending output before raw mode. There are no writes in raw
    // mode, so a closed output pipe cannot stop the process in raw mode.
    let _ = io::stdout().flush();

    let stdin = io::stdin();
    let raw_mode = if stdin.is_terminal() {
        RawMode::enable()
    } else {
        None
    };
    let key = read(&mut stdin.lock());
    let was_raw = raw_mode.is_some();
    drop(raw_mode);

    // Raw mode stops Ctrl-C as a signal. Stop the process as the signal does.
    if was_raw && key == Some(CTRL_C) {
        std::process::exit(130);
    }
    key
}

/// Read one byte. Give None at the end of input or on a read error.
fn read_byte(input: &mut impl Read) -> Option<u8> {
    let mut byte = [0u8; 1];
    loop {
        match input.read(&mut byte) {
            Ok(1) => return Some(byte[0]),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            _ => return None,
        }
    }
}

#[cfg(unix)]
mod timed {
    use std::io::{self, Read};
    use std::os::fd::{AsRawFd, RawFd};
    use std::time::{Duration, Instant};

    /// Read one byte from `input`, but wait for a maximum of `limit`.
    ///
    /// The file descriptor is non-blocking during the read. Thus a byte in
    /// the buffer of `input` comes at once, and an empty input gives
    /// WouldBlock. Then poll waits until the input is ready or the time ends.
    pub(super) fn read_byte_within(
        input: &mut (impl Read + AsRawFd),
        limit: Duration,
    ) -> Option<u8> {
        let fd = input.as_raw_fd();
        let deadline = Instant::now() + limit;
        let _non_blocking = NonBlocking::enable(fd)?;
        let mut byte = [0u8; 1];
        loop {
            match input.read(&mut byte) {
                Ok(1) => return Some(byte[0]),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() || !wait_until_readable(fd, remaining) {
                        return None;
                    }
                }
                // Ok(0) is the end of input. Do not wait for the time limit.
                _ => return None,
            }
        }
    }

    /// Wait for a maximum of `limit` until `fd` has data or is at its end.
    /// Give false when the time ends or on an error.
    fn wait_until_readable(fd: RawFd, limit: Duration) -> bool {
        let mut poll_fd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        // Round up, so that a time of less than 1 ms does not become 0.
        let milliseconds = limit.as_micros().div_ceil(1000);
        let timeout = libc::c_int::try_from(milliseconds).unwrap_or(libc::c_int::MAX);
        // SAFETY: poll_fd is one valid pollfd for the duration of the call.
        match unsafe { libc::poll(&mut poll_fd, 1, timeout) } {
            ready if ready > 0 => true,
            0 => false,
            // A signal stopped the wait. The caller reads again and then
            // waits for the time that remains.
            _ => io::Error::last_os_error().kind() == io::ErrorKind::Interrupted,
        }
    }

    /// The file descriptor is non-blocking while this value exists. Drop
    /// restores the old flags, also when a panic unwinds.
    struct NonBlocking {
        fd: RawFd,
        old_flags: libc::c_int,
    }

    impl NonBlocking {
        /// Make `fd` non-blocking. Give None if fcntl fails.
        fn enable(fd: RawFd) -> Option<Self> {
            // SAFETY: F_GETFL and F_SETFL only read and write the file status
            // flags. An invalid fd gives -1, which is examined below.
            let old_flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
            if old_flags == -1 {
                return None;
            }
            if unsafe { libc::fcntl(fd, libc::F_SETFL, old_flags | libc::O_NONBLOCK) } == -1 {
                return None;
            }
            Some(NonBlocking { fd, old_flags })
        }
    }

    impl Drop for NonBlocking {
        fn drop(&mut self) {
            // SAFETY: this restores the flags that enable read from this fd.
            unsafe { libc::fcntl(self.fd, libc::F_SETFL, self.old_flags) };
        }
    }
}

#[cfg(not(unix))]
mod timed {
    use std::io::Read;
    use std::time::Duration;

    /// This build has no timed read for this platform. The read waits for a
    /// key with no time limit, as GET does.
    pub(super) fn read_byte_within(input: &mut impl Read, _limit: Duration) -> Option<u8> {
        super::read_byte(input)
    }
}

/// The terminal is in raw mode while this value exists. Drop restores the
/// mode, also when a panic unwinds.
struct RawMode;

impl RawMode {
    /// Enable raw mode. Give None if the terminal refuses, so that the read
    /// uses line mode.
    fn enable() -> Option<Self> {
        crossterm::terminal::enable_raw_mode()
            .ok()
            .map(|()| RawMode)
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
    }
}

#[cfg(test)]
mod tests {
    use super::read_byte;

    #[test]
    fn test_read_byte_gives_bytes_in_order_then_none() {
        let mut input: &[u8] = b"AB";

        assert_eq!(read_byte(&mut input), Some(b'A'));
        assert_eq!(read_byte(&mut input), Some(b'B'));
        assert_eq!(read_byte(&mut input), None);
    }

    #[cfg(unix)]
    mod timed {
        use super::super::timed::read_byte_within;
        use std::io::Write;
        use std::os::unix::net::UnixStream;
        use std::time::{Duration, Instant};

        #[test]
        fn test_read_byte_within_gives_a_ready_byte_with_no_wait() {
            let (mut writer, mut reader) = UnixStream::pair().unwrap();
            writer.write_all(b"K").unwrap();
            let start = Instant::now();

            assert_eq!(
                read_byte_within(&mut reader, Duration::from_secs(3)),
                Some(b'K')
            );
            assert!(start.elapsed() < Duration::from_millis(500));
        }

        #[test]
        fn test_read_byte_within_waits_the_limit_when_no_byte_comes() {
            let (_writer, mut reader) = UnixStream::pair().unwrap();
            let start = Instant::now();

            assert_eq!(
                read_byte_within(&mut reader, Duration::from_millis(200)),
                None
            );
            let elapsed = start.elapsed();
            assert!(elapsed >= Duration::from_millis(200), "took {elapsed:?}");
            assert!(elapsed < Duration::from_secs(2), "took {elapsed:?}");
        }

        #[test]
        fn test_read_byte_within_gives_a_byte_that_comes_during_the_wait() {
            let (mut writer, mut reader) = UnixStream::pair().unwrap();
            let sender = std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(100));
                writer.write_all(b"Z").unwrap();
                writer
            });

            assert_eq!(
                read_byte_within(&mut reader, Duration::from_secs(3)),
                Some(b'Z')
            );
            drop(sender.join().unwrap());
        }

        #[test]
        fn test_read_byte_within_at_end_of_input_gives_none_with_no_wait() {
            let (writer, mut reader) = UnixStream::pair().unwrap();
            drop(writer);
            let start = Instant::now();

            assert_eq!(read_byte_within(&mut reader, Duration::from_secs(3)), None);
            assert!(start.elapsed() < Duration::from_millis(500));
        }

        #[test]
        fn test_read_byte_within_restores_blocking_mode() {
            use std::os::fd::AsRawFd;
            let (_writer, mut reader) = UnixStream::pair().unwrap();
            let fd = reader.as_raw_fd();
            // SAFETY: F_GETFL only reads the flags of a valid fd.
            let flags = || unsafe { libc::fcntl(fd, libc::F_GETFL) };
            let before = flags();

            read_byte_within(&mut reader, Duration::ZERO);

            assert_eq!(flags(), before);
            assert_eq!(before & libc::O_NONBLOCK, 0);
        }
    }
}
