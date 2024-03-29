// Copyright (C) 2019-2021 O.S. Systems Sofware LTDA
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::VersionFinder;
use regex::bytes::Regex;
use std::{io::SeekFrom, str};
use tokio::io::{self, AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt};

#[allow(clippy::enum_variant_names, clippy::upper_case_acronyms)]
enum LinuxKernelKind {
    ARMzImage,
    UImage,
    X86bzImage,
    X86zImage,
}

// U-Boot Image Magic Number
const UIMAGE_MAGIC_NUMBER: u32 = 0x2705_1956;

// zImage Magic Number used in ARM
const ARM_ZIMAGE_MAGIC_NUMBER: u32 = 0x016F_2818;

async fn discover_linux_kernel_kind<R: AsyncRead + AsyncSeek + Unpin>(
    buf: &mut R,
) -> Option<LinuxKernelKind> {
    // U-Boot Image Magic header is stored at begin of file
    buf.seek(SeekFrom::Start(0x0000)).await.ok()?;
    if buf.read_u32().await.ok()? == UIMAGE_MAGIC_NUMBER {
        return Some(LinuxKernelKind::UImage);
    }

    // ARM zImage Magic header is stored at offset 0x0024 of file
    buf.seek(SeekFrom::Start(0x0024)).await.ok()?;
    if buf.read_u32_le().await.ok()? == ARM_ZIMAGE_MAGIC_NUMBER {
        return Some(LinuxKernelKind::ARMzImage);
    }

    // Taken from: https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/Documentation/x86/boot.txt#n144
    //
    // Offset  Proto   Name            Meaning
    // /Size
    // ...
    // 01FE/2  ALL     boot_flag       0xAA55 magic number
    // ...
    // 0211/1	2.00+	loadflags	Boot protocol option flags

    // Verify the boot_flag magic number
    buf.seek(SeekFrom::Start(0x01FE)).await.ok()?;
    if buf.read_u16_le().await.ok()? != 0xAA55 {
        return None;
    }

    // Field name:	loadflags
    // Type:		modify (obligatory)
    // Offset/size:	0x211/1
    // Protocol:	2.00+
    //
    //   This field is a bitmask.
    //
    //   Bit 0 (read):	LOADED_HIGH
    //         - If 0, the protected-mode code is loaded at 0x10000.
    //         - If 1, the protected-mode code is loaded at 0x100000.
    //   ...
    buf.seek(SeekFrom::Start(0x0211)).await.ok()?;
    match buf.read_u8().await.ok()? & 0x1 {
        0 => Some(LinuxKernelKind::X86zImage),
        1 => Some(LinuxKernelKind::X86bzImage),
        _ => None,
    }
}

pub(crate) struct LinuxKernel<'a, R: AsyncRead + AsyncSeek + Unpin> {
    buf: &'a mut R,
}

impl<'a, R: AsyncRead + AsyncSeek + Unpin> LinuxKernel<'a, R> {
    pub(crate) fn from_reader(buf: &'a mut R) -> Self {
        LinuxKernel { buf }
    }
}

#[async_trait::async_trait(?Send)]
impl<'a, R: AsyncRead + AsyncSeek + Unpin> VersionFinder for LinuxKernel<'a, R> {
    async fn get_version(&mut self) -> Option<String> {
        match discover_linux_kernel_kind(self.buf).await? {
            LinuxKernelKind::ARMzImage => {
                async fn get_version_from_arm<R: AsyncRead + Unpin>(mut rd: R) -> Option<String> {
                    let mut buffer = Vec::default();
                    compress_tools::tokio_support::uncompress_data(&mut rd, &mut buffer)
                        .await
                        .ok()?;
                    let re = Regex::new(r"Linux version (?P<version>\S+).*").unwrap();
                    re.captures(&buffer)
                        .and_then(|m| m.name("version"))
                        .and_then(|v| str::from_utf8(v.as_bytes()).ok())
                        .map(|v| v.to_string())
                }

                let mut buffer = [0; 0x200];
                loop {
                    let n = self.buf.read(&mut buffer).await.ok()?;

                    // No more data to read
                    if n == 0 {
                        return None;
                    }

                    // Look for compression format header
                    for (offset, window) in buffer[0..n].windows(6).enumerate() {
                        // Headers taken from:
                        // https://github.com/torvalds/linux/blob/master/scripts/extract-vmlinux
                        match window {
                            [0x1f, 0x8b, 0x08, ..] => {}               // gzip
                            [0xfd, b'7', b'z', b'X', b'Z', 0x00] => {} // xz
                            [b'B', b'Z', b'h', ..] => {}               // bzip2
                            [0x5d, 0x00, 0x00, ..] => {}               // lzma
                            [0x89, 0x4c, 0x5a, ..] => {}               // lzo
                            [0x02, b'!', b'L', 0x18, ..] => {}         // lz4
                            [b'(', 0xb5, b'/', 0xfd, ..] => {}         // zstd
                            _ => continue,
                        }

                        let mut slice = &buffer[offset..];
                        let current = self.buf.seek(SeekFrom::Current(0)).await.ok()?;
                        let rd = io::AsyncReadExt::chain(&mut slice, &mut self.buf);

                        // Try to get version from uncompressed data
                        if let Some(version) = get_version_from_arm(rd).await {
                            return Some(version);
                        }

                        // Seek back to current position so we can keep looking
                        // for the next compression header
                        self.buf.seek(SeekFrom::Start(current)).await.ok()?;
                    }
                }
            }

            LinuxKernelKind::X86bzImage | LinuxKernelKind::X86zImage => {
                // Taken from: https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/Documentation/x86/boot.txt#n144
                //
                // Offset  Proto   Name            Meaning
                // /Size
                // ...
                // 01F1/1  ALL(1   setup_sects     The size of the setup in sectors
                // ...
                // 020E/2  2.00+   kernel_version  Pointer to kernel version string

                // Get the setup_sects information
                self.buf.seek(SeekFrom::Start(0x01F1)).await.ok()?;
                let setup_sects = u64::from(self.buf.read_u8().await.ok()?);

                // Get kernel_version pointer
                self.buf.seek(SeekFrom::Start(0x020E)).await.ok()?;
                let kernel_version_ptr = u64::from(self.buf.read_u16_le().await.ok()?);

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
                    .await
                    .ok()?;

                // Read the Linux kernel version from the reader
                let mut buffer = [0; 0x200];
                let _ = self.buf.read(&mut buffer).await.ok()?;

                let re = Regex::new(r"(?P<version>\d+.?\.[^\s\u{0}]+)").unwrap();
                re.captures(&buffer)
                    .and_then(|m| m.name("version"))
                    .and_then(|v| str::from_utf8(v.as_bytes()).ok())
                    .map(|v| v.to_string())
            }

            LinuxKernelKind::UImage => {
                // Move to the begin of the file, so we can next read the
                // buffer to match the version.
                self.buf.seek(SeekFrom::Start(0)).await.ok()?;

                // Read the Linux kernel version from the reader
                let mut buffer = [0; 0x200];
                let _ = self.buf.read(&mut buffer).await.ok()?;

                let re = Regex::new(r"(?P<version>\d+.?\.[^\s\u{0}]+)").unwrap();
                re.captures(&buffer)
                    .and_then(|m| m.name("version"))
                    .and_then(|v| str::from_utf8(v.as_bytes()).ok())
                    .map(|v| v.to_string())
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{version, BinaryKind};
    use tokio::io::{AsyncRead, AsyncSeek};

    async fn fixture(name: &str) -> impl AsyncRead + AsyncSeek {
        use tokio::{fs::File, io::BufReader};

        BufReader::new(
            File::open(&format!("tests/fixtures/linuxkernel/{}", name))
                .await
                .unwrap_or_else(|_| panic!("Couldn't open the fixture {}", name)),
        )
    }

    #[tokio::test]
    async fn linux_version() {
        for (f, v) in &[
            ("arm-uImage", "4.1.15-1.2.0+g274a055"),
            ("arm-zImage", "4.4.1"),
            ("x86-bzImage", "4.1.30-1-MANJARO"),
            ("x86-zImage", "4.1.30-1-MANJARO"),
        ] {
            assert_eq!(
                version(&mut fixture(f).await, BinaryKind::LinuxKernel).await,
                Some(v.to_string())
            );
        }
    }
}
