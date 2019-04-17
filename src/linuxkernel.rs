// Copyright (C) 2019 O.S. Systems Sofware LTDA
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::VersionFinder;
use byteorder::{LittleEndian, ReadBytesExt};
use regex::bytes::Regex;
use std::io::{Read, Seek, SeekFrom};

pub(crate) struct LinuxKernel<'a, R>
where
    R: Read + Seek,
{
    buf: &'a mut R,
}

impl<'a, R: Read + Seek> LinuxKernel<'a, R> {
    pub(crate) fn from_reader(buf: &'a mut R) -> Self
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
        if self.buf.read_u16::<LittleEndian>().ok()? != 0xAA55 {
            return None;
        }

        // Get kernel_version pointer
        self.buf.seek(SeekFrom::Start(0x020E)).ok()?;
        let kernel_version_ptr = u64::from(self.buf.read_u16::<LittleEndian>().ok()?);

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

        // Read the Linux kernel version from the reader
        let mut buffer = [0; 0x200];
        self.buf.read(&mut buffer).ok()?;

        // Filter out unnecessary information
        let re = Regex::new(r"(?P<version>\d+.?\.[^\s]+)").unwrap();
        re.captures(&buffer)
            .and_then(|m| m.name("version"))
            .and_then(|v| std::str::from_utf8(v.as_bytes()).ok())
            .and_then(|v| Some(v.to_string()))
    }
}

#[cfg(test)]
mod test {
    use crate::{version, BinaryKind};
    use std::io::{Read, Seek};

    fn fixture(name: &str) -> impl Read + Seek {
        use std::{fs::File, io::BufReader};

        BufReader::new(
            File::open(&format!("tests/fixtures/linuxkernel/{}", name))
                .unwrap_or_else(|_| panic!("Couldn't open the fixture {}", name)),
        )
    }

    #[test]
    fn linux_version() {
        for (f, v) in &[
            ("x86-bzImage", "4.1.30-1-MANJARO"),
            ("x86-zImage", "4.1.30-1-MANJARO"),
        ] {
            assert_eq!(
                version(BinaryKind::LinuxKernel, &mut fixture(f)),
                Some(v.to_string())
            );
        }
    }
}
