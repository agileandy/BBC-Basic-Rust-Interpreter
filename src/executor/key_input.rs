//! Read one key from stdin for GET and GET$.
//!
//! On a terminal, the read uses raw mode, so that the key comes with no
//! Return key and no echo. With piped input, the read takes the next byte.

use std::io::{self, IsTerminal, Read, Write};

/// The byte that Ctrl-C gives in raw mode.
const CTRL_C: u8 = 0x03;

/// Read one key from stdin. Give None at the end of input or on a read error.
///
/// The read goes through the shared stdin buffer, so it takes the next byte
/// after the lines that the REPL and INPUT already read.
pub(crate) fn read_key() -> Option<u8> {
    // Write the pending output before raw mode. There are no writes in raw
    // mode, so a closed output pipe cannot stop the process in raw mode.
    let _ = io::stdout().flush();

    let stdin = io::stdin();
    let raw_mode = if stdin.is_terminal() {
        RawMode::enable()
    } else {
        None
    };
    let key = read_byte(&mut stdin.lock());
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
}
