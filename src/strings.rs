// Copyright (C) 2016 Ticki
// Copyright (C) 2019 O.S. Systems Sofware LTDA
//
// SPDX-License-Identifier: MIT OR Apache-2.0

// This code is based on Redox OS implementation of binutils' strings
// module. This reworked the code to provide an Iterator over the
// bytes so it allows a more flexible use.
//
// Reference code:
//  https://gitlab.redox-os.org/redox-os/binutils/blob/966c6f039e20d56cec369621065646c4f21cbd61/src/strings.rs

use std::{io::Read, slice, str};

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

/// A buffer tracking the previous printable characters.
#[derive(Copy, Clone)]
struct Trailing {
    chars: [u8; 4],
    current: usize,
}

#[allow(dead_code)]
impl Trailing {
    #[inline]
    fn new() -> Trailing {
        Trailing {
            chars: [0; 4],
            current: 0,
        }
    }

    #[inline]
    fn set(&mut self, b: u8) -> bool {
        self.chars[self.current] = b;
        self.current += 1;

        self.is_complete()
    }

    #[inline]
    fn reset(&mut self) {
        self.current = 0;
    }

    #[inline]
    fn is_complete(self) -> bool {
        self.current == 4
    }

    #[inline]
    fn chars(self) -> [u8; 4] {
        self.chars
    }
}

/// Wraps a reader to provide a strings iterator.
pub(crate) struct Strings<R>(R);

pub(crate) trait IntoStringsIter<T> {
    fn into_strings_iter(self) -> Strings<T>;
}

impl<T: Read> IntoStringsIter<T> for T {
    fn into_strings_iter(self) -> Strings<T> {
        Strings(self)
    }
}

/// Provides an iterator to a stream of bytes and output printable
/// strings of length 4 or more.
impl<R: Read> Iterator for Strings<R> {
    type Item = String;

    fn next(&mut self) -> Option<String> {
        let mut stanza = String::new();
        let mut trailing = Trailing::new();
        let mut byte = 0;

        loop {
            match self.0.read(slice::from_mut(&mut byte)) {
                Ok(0) => {
                    if stanza.is_empty() {
                        return None;
                    }

                    return Some(stanza);
                }
                Ok(_) => {
                    if byte.is_printable() {
                        if trailing.is_complete() {
                            stanza.push_str(str::from_utf8(&[byte]).unwrap());
                        } else if trailing.set(byte) {
                            stanza.push_str(str::from_utf8(&trailing.chars()).unwrap());
                        }
                    } else {
                        if trailing.is_complete() {
                            return Some(stanza);
                        }

                        trailing.reset();
                    }
                }
                _ => continue,
            };
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

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

    #[test]
    fn iterator() {
        let bytes = std::io::Cursor::new(b"\0\tfoobar\r\tbarfoo");
        let mut bytes = bytes.into_strings_iter();

        assert_eq!(Some("foobar".to_string()), bytes.next());
        assert_eq!(Some("barfoo".to_string()), bytes.next());
        assert_eq!(None, bytes.next());
    }
}
