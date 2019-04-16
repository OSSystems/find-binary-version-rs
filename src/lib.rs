// Copyright (C) 2019 O.S. Systems Sofware LTDA
//
// SPDX-License-Identifier: MIT OR Apache-2.0

#![deny(
    missing_copy_implementations,
    missing_debug_implementations,
    missing_docs,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unstable_features,
    unused_import_braces,
    unused_qualifications,
    warnings
)]

//! The library provide a way to get the binary version for a specific
//! binary.

use crate::strings::IntoStringsIter;
use byteorder::{BigEndian, ReadBytesExt};
use regex::Regex;
use std::{
    ffi::OsStr,
    io::{Read, Seek, SeekFrom},
    os::unix::ffi::OsStrExt,
};

mod strings;

#[derive(Debug, Copy, Clone)]
/// Define the binary kind to use for matching.
pub enum BinaryKind {
    /// U-Boot binary kind.
    UBoot,

    /// Linux Kernel binary kind.
    LinuxKernel,
}

trait VersionFinder {
    fn get_version(&mut self) -> Option<String>;
}

/// Get the version for a specific binary.
pub fn version<R>(kind: BinaryKind, mut buffer: &mut R) -> Option<String>
where
    R: Read + Seek,
{
    match kind {
        BinaryKind::UBoot => UBoot::from_reader(&mut buffer).get_version(),
        BinaryKind::LinuxKernel => LinuxKernel::from_reader(&mut buffer).get_version(),
    }
}

struct UBoot<'a, R> {
    buf: &'a mut R,
}

impl<'a, R> UBoot<'a, R> {
    pub fn from_reader(buf: &'a mut R) -> Self {
        UBoot { buf }
    }
}

impl<'a, R: Read> VersionFinder for UBoot<'a, R> {
    fn get_version(&mut self) -> Option<String> {
        let re = Regex::new(r"U-Boot(?: SPL)? (?P<version>\S+) \(.*\)").unwrap();
        for stanza in self.buf.into_strings_iter() {
            if let Some(v) = re.captures(&stanza).and_then(|m| m.name("version")) {
                return Some(v.as_str().to_string());
            }
        }

        None
    }
}

struct LinuxKernel<'a, R>
where
    R: Read + Seek,
{
    buf: &'a mut R,
}

impl<'a, R: Read + Seek> LinuxKernel<'a, R> {
    fn from_reader(buf: &'a mut R) -> Self
    where
        R: Read,
    {
        LinuxKernel { buf }
    }
}

impl<'a, R: Read + Seek> VersionFinder for LinuxKernel<'a, R> {
    fn get_version(&mut self) -> Option<String> {
        // Taken from: https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/Documentation/x86/boot.txt#n144
        //
        // Offset  Proto   Name            Meaning
        // /Size
        // ...
        // 01F1/1  ALL(1   setup_sects     The size of the setup in sectors
        // ...
        // 01FE/2  ALL     boot_flag       0xAA55 magic number
        // ...
        // 020E/2  2.00+   kernel_version  Pointer to kernel version string

        self.buf.seek(SeekFrom::Start(0x01F1)).ok()?;
        let setup_sects = u64::from(self.buf.read_u8().ok()?);

        // Verify the boot_flag magic number
        self.buf.seek(SeekFrom::Start(0x01FE)).ok()?;
        if self.buf.read_u16::<BigEndian>().ok()? != 0xAA55 {
            return None;
        }

        // Get kernel_version pointer
        self.buf.seek(SeekFrom::Start(0x020E)).ok()?;
        let kernel_version_ptr = u64::from(self.buf.read_u16::<BigEndian>().ok()?);

        // Field name:     kernel_version
        // Type:           read
        // Offset/size:    0x20e/2
        // Protocol:       2.00+
        //
        //   If set to a nonzero value, contains a pointer to a NUL-terminated
        //   human-readable kernel version number string, less 0x200.  This can
        //   be used to display the kernel version to the user.  This value
        //   should be less than (0x200*setup_sects).
        if kernel_version_ptr >= setup_sects * 0x200 {
            return None;
        }

        // Move to the kernel version location
        self.buf
            .seek(SeekFrom::Start(kernel_version_ptr + 0x200))
            .ok()?;

        let buf = self
            .buf
            .bytes()
            .map(|c| c.unwrap_or(0))
            .take_while(|&n| n != 0)
            .collect::<Vec<u8>>();

        Some(OsStr::from_bytes(&buf).to_str()?.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use byteorder::WriteBytesExt;
    use std::io::{Cursor, Write};

    #[test]
    fn u_boot() {
        for (content, expected) in vec![
            ("U-Boot 2019.04 (01/04/2019)", Some("2019.04".to_string())),
            (
                "U-Boot SPL 2019.04 (01/04/2019)",
                Some("2019.04".to_string()),
            ),
        ] {
            assert_eq!(
                version(BinaryKind::UBoot, &mut Cursor::new(content.as_bytes())),
                expected,
                "Failed to parse {:?}",
                content
            );
        }
    }

    #[test]
    fn linux_version() {
        let mut buf = Cursor::new(Vec::new());
        // Write the setup_sects
        buf.set_position(0x01F1);
        buf.write_u8(15).unwrap();

        // Write the magic number
        buf.set_position(0x01FE);
        buf.write_u16::<BigEndian>(0xAA55).unwrap();

        // Write the version offset
        buf.set_position(0x020E);
        buf.write_u16::<BigEndian>(0x1C00).unwrap();

        // Write the version data
        buf.set_position(0x1C00 + 0x200);
        buf.write_all(b"5.0.8").unwrap();

        assert_eq!(
            version(BinaryKind::LinuxKernel, &mut buf),
            Some("5.0.8".to_string())
        );
    }
}
