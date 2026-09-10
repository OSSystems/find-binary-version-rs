// Copyright (C) 2019-2021 O.S. Systems Sofware LTDA
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::VersionFinder;
use regex::bytes::Regex;
use std::{io::SeekFrom, str};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt};

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

/// How much is read at a time. Kept small because this runs on embedded
/// devices, where reading a whole image into memory is not an option.
const WINDOW: usize = 0x200;

/// Length of the longest compression header looked for below. A window carries
/// one byte less than this into the next one, so a header landing across a read
/// boundary is still seen whole.
const MAGIC_LEN: usize = 6;

/// Reads until `buf` is full or the reader runs out, since a single read is
/// free to come up short while more data is still available. Returns how much
/// of `buf` holds data.
async fn read_filled<R: AsyncRead + Unpin>(rd: &mut R, buf: &mut [u8]) -> Option<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        let n = rd.read(&mut buf[filled..]).await.ok()?;
        if n == 0 {
            break;
        }
        filled += n;
    }
    Some(filled)
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

                let mut buffer = [0; MAGIC_LEN - 1 + WINDOW];

                // Bytes held over from the previous window, sitting at the
                // front of the buffer.
                let mut carried = 0;

                loop {
                    let n = self.buf.read(&mut buffer[carried..]).await.ok()?;

                    // No more data to read
                    if n == 0 {
                        return None;
                    }
                    let filled = carried + n;

                    // Look for compression format header. Only the bytes this
                    // pass holds may be looked at: anything past them is left
                    // over from an earlier window.
                    for (offset, window) in buffer[..filled].windows(MAGIC_LEN).enumerate() {
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

                        let mut slice = &buffer[offset..filled];
                        let current = self.buf.seek(SeekFrom::Current(0)).await.ok()?;
                        let rd = AsyncReadExt::chain(&mut slice, &mut self.buf);

                        // Try to get version from uncompressed data
                        if let Some(version) = get_version_from_arm(rd).await {
                            return Some(version);
                        }

                        // Seek back to current position so we can keep looking
                        // for the next compression header
                        self.buf.seek(SeekFrom::Start(current)).await.ok()?;
                    }

                    // Keep the tail for the next pass, so a header the read cut
                    // in half is found once its remainder arrives.
                    carried = filled.min(MAGIC_LEN - 1);
                    buffer.copy_within(filled - carried..filled, 0);
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
                let mut buffer = [0; WINDOW];
                let filled = read_filled(self.buf, &mut buffer).await?;

                let re = Regex::new(r"(?P<version>\d+.?\.[^\s\u{0}]+)").unwrap();
                re.captures(&buffer[..filled])
                    .and_then(|m| m.name("version"))
                    .and_then(|v| str::from_utf8(v.as_bytes()).ok())
                    .map(|v| v.to_string())
            }

            LinuxKernelKind::UImage => {
                // Move to the begin of the file, so we can next read the
                // buffer to match the version.
                self.buf.seek(SeekFrom::Start(0)).await.ok()?;

                // Read the Linux kernel version from the reader
                let mut buffer = [0; WINDOW];
                let filled = read_filled(self.buf, &mut buffer).await?;

                let re = Regex::new(r"(?P<version>\d+.?\.[^\s\u{0}]+)").unwrap();
                re.captures(&buffer[..filled])
                    .and_then(|m| m.name("version"))
                    .and_then(|v| str::from_utf8(v.as_bytes()).ok())
                    .map(|v| v.to_string())
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::WINDOW;
    use crate::{version, BinaryKind};
    use std::{
        io::{Cursor, Result, Seek, SeekFrom},
        pin::Pin,
        task::{Context, Poll},
    };
    use tokio::io::{AsyncRead, AsyncSeek, ReadBuf};

    async fn fixture(name: &str) -> impl AsyncRead + AsyncSeek {
        use tokio::{fs::File, io::BufReader};

        BufReader::new(
            File::open(&format!("tests/fixtures/linuxkernel/{name}"))
                .await
                .unwrap_or_else(|_| panic!("Couldn't open the fixture {name}")),
        )
    }

    fn fixture_bytes(name: &str) -> Vec<u8> {
        std::fs::read(format!("tests/fixtures/linuxkernel/{name}"))
            .unwrap_or_else(|_| panic!("Couldn't open the fixture {name}"))
    }

    /// A reader handing out at most `chunk` bytes per read, the way a pipe or a
    /// slow device does.
    struct Trickle {
        inner: Cursor<Vec<u8>>,
        chunk: usize,
    }

    impl Trickle {
        fn new(data: Vec<u8>, chunk: usize) -> Self {
            Trickle {
                inner: Cursor::new(data),
                chunk,
            }
        }
    }

    impl AsyncRead for Trickle {
        fn poll_read(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<Result<()>> {
            let n = self.chunk.min(buf.remaining());
            let mut chunk = vec![0; n];
            let read = std::io::Read::read(&mut self.inner, &mut chunk)?;
            buf.put_slice(&chunk[..read]);
            Poll::Ready(Ok(()))
        }
    }

    impl AsyncSeek for Trickle {
        fn start_seek(mut self: Pin<&mut Self>, pos: SeekFrom) -> Result<()> {
            self.inner.seek(pos).map(|_| ())
        }

        fn poll_complete(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<u64>> {
            Poll::Ready(self.inner.stream_position())
        }
    }

    /// The compressed payload must be found wherever it lands, including across
    /// the boundary between two reads.
    #[tokio::test]
    async fn compression_header_split_across_reads() {
        // The fixture keeps its gzip header right after the zImage magic, and
        // the search starts reading where the magic check left off, so windows
        // fall on multiples of `WINDOW` counted from there. Push the header out
        // to one of those boundaries without disturbing the magic itself.
        const HEADER_AT: usize = 0x28;
        let original = fixture_bytes("arm-zImage");

        for target in [
            HEADER_AT + WINDOW - 2,
            HEADER_AT + WINDOW - 1,
            HEADER_AT + WINDOW,
            HEADER_AT + 2 * WINDOW - 3,
        ] {
            let mut data = original[..HEADER_AT].to_vec();
            data.resize(target, 0);
            data.extend_from_slice(&original[HEADER_AT..]);

            assert_eq!(
                version(&mut Cursor::new(data), BinaryKind::LinuxKernel).await,
                Some("4.4.1".to_string()),
                "payload at offset {target:#x} was not found",
            );
        }
    }

    /// A short read must not truncate the version, which would report a version
    /// that no kernel ever had.
    #[tokio::test]
    async fn version_from_a_reader_with_short_reads() {
        for (f, v) in &[
            ("arm-uImage", "4.1.15-1.2.0+g274a055"),
            ("x86-bzImage", "4.1.30-1-MANJARO"),
            ("x86-zImage", "4.1.30-1-MANJARO"),
        ] {
            for chunk in [1, 7, 64, 333] {
                assert_eq!(
                    version(
                        &mut Trickle::new(fixture_bytes(f), chunk),
                        BinaryKind::LinuxKernel
                    )
                    .await,
                    Some(v.to_string()),
                    "{f} read {chunk} bytes at a time",
                );
            }
        }
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
