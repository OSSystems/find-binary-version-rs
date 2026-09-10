// Copyright (C) 2016 Ticki
// Copyright (C) 2019-2021 O.S. Systems Sofware LTDA
//
// SPDX-License-Identifier: MIT OR Apache-2.0

// This code is based on Redox OS implementation of binutils' strings
// module. This reworked the code to read the source in windows, so a
// caller never has to hold the whole of it in memory.
//
// Reference code:
//  https://gitlab.redox-os.org/redox-os/binutils/blob/966c6f039e20d56cec369621065646c4f21cbd61/src/strings.rs

use tokio::io::{AsyncRead, AsyncReadExt};

/// How much is read at a time. Kept small because this runs on embedded
/// devices, where reading a whole image into memory is not an option.
const WINDOW: usize = 0x200;

/// Shortest run of printable characters worth reporting, matching what
/// binutils' `strings` settles on.
const MIN_LEN: usize = 4;

/// A trait for characters/bytes that can be printable.
pub(crate) trait IsPrintable {
    /// Is this character printable?
    fn is_printable(&self) -> bool;
}

impl IsPrintable for u8 {
    #[inline]
    fn is_printable(&self) -> bool {
        // Is an ASCII in a printable range
        (0x20..=0x7e).contains(self)
    }
}

/// Walks a reader and hands back the printable runs in it, one at a time.
///
/// Only the window being scanned and the run being built are held, so the
/// source is never read into memory whole.
pub(crate) struct Strings<'a, R: AsyncRead + Unpin> {
    buf: &'a mut R,
    window: [u8; WINDOW],
    /// How much of `window` the last read filled, and how far into it the scan
    /// has reached.
    filled: usize,
    pos: usize,
}

impl<'a, R: AsyncRead + Unpin> Strings<'a, R> {
    pub(crate) fn from_reader(buf: &'a mut R) -> Self {
        Strings {
            buf,
            window: [0; WINDOW],
            filled: 0,
            pos: 0,
        }
    }

    /// The next run of [`MIN_LEN`] or more printable characters, or `None` once
    /// the reader has nothing left to give.
    pub(crate) async fn next_string(&mut self) -> Option<String> {
        let mut stanza = String::new();

        loop {
            if self.pos == self.filled {
                self.filled = self.buf.read(&mut self.window).await.ok()?;
                self.pos = 0;

                // Nothing more to read, so whatever has been built is all there
                // is to report.
                if self.filled == 0 {
                    return if stanza.len() >= MIN_LEN {
                        Some(stanza)
                    } else {
                        None
                    };
                }
            }

            let byte = self.window[self.pos];
            self.pos += 1;

            if byte.is_printable() {
                stanza.push(char::from(byte));
            } else if stanza.len() >= MIN_LEN {
                return Some(stanza);
            } else {
                // Too short to be worth reporting, so start over on the next
                // run rather than carrying it.
                stanza.clear();
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn printable() {
        assert!(!b'\0'.is_printable());
        assert!(!b'\t'.is_printable());
        assert!(!b'\n'.is_printable());
        assert!(!b'\r'.is_printable());
        assert!(!b'\x1b'.is_printable());
        assert!(b'a'.is_printable());
        assert!(b'B'.is_printable());
        assert!(b'x'.is_printable());
        assert!(b'~'.is_printable());
    }

    #[tokio::test]
    async fn runs_between_unprintable_bytes() {
        let mut bytes = Cursor::new(b"\0\tfoobar\r\tbarfoo".to_vec());
        let mut strings = Strings::from_reader(&mut bytes);

        assert_eq!(Some("foobar".to_string()), strings.next_string().await);
        assert_eq!(Some("barfoo".to_string()), strings.next_string().await);
        assert_eq!(None, strings.next_string().await);
    }

    #[tokio::test]
    async fn runs_shorter_than_the_minimum_are_dropped() {
        let mut bytes = Cursor::new(b"\0abc\0abcd\0".to_vec());
        let mut strings = Strings::from_reader(&mut bytes);

        assert_eq!(Some("abcd".to_string()), strings.next_string().await);
        assert_eq!(None, strings.next_string().await);
    }

    /// A run is not cut short by the window it happens to straddle.
    #[tokio::test]
    async fn run_spanning_several_windows() {
        let run = "A".repeat(2 * WINDOW + 16);
        let mut data = vec![0];
        data.extend_from_slice(run.as_bytes());
        data.push(0);

        let mut bytes = Cursor::new(data);
        let mut strings = Strings::from_reader(&mut bytes);

        assert_eq!(Some(run), strings.next_string().await);
        assert_eq!(None, strings.next_string().await);
    }
}
