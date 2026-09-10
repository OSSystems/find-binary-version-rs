// Copyright (C) 2019-2021 O.S. Systems Sofware LTDA
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::VersionFinder;
use regex::bytes::Regex;
use std::str;
use tokio::io::{AsyncRead, AsyncReadExt};

pub(crate) struct UBoot<'a, R: AsyncRead + Unpin> {
    buf: &'a mut R,
}

impl<'a, R: AsyncRead + Unpin> UBoot<'a, R> {
    pub(crate) fn from_reader(buf: &'a mut R) -> Self {
        UBoot { buf }
    }
}

/// How much of each window is carried into the next one, so a banner landing
/// across a read boundary is still seen whole. U-Boot banners run to about
/// eighty bytes, which this leaves ample room for.
const OVERLAP: usize = 0x100;

/// How much is read at a time. Kept small because this runs on embedded
/// devices, where reading a whole image into memory is not an option.
const WINDOW: usize = 0x200;

#[async_trait::async_trait(?Send)]
impl<'a, R: AsyncRead + Unpin> VersionFinder for UBoot<'a, R> {
    async fn get_version(&mut self) -> Option<String> {
        // We use a fixed size buffer to avoid allocing too much memory on
        // embedded devices.
        let mut buffer = [0; OVERLAP + WINDOW];

        // Bytes held over from the previous window, sitting at the front of the
        // buffer.
        let mut carried = 0;

        // Avoid recompiling the pattern.
        let re = Regex::new(r"U-Boot(?: SPL)? (?P<version>\d+.?\.[^\s]+) \(.*\)").unwrap();

        // Read the U-Boot version from the reader.
        loop {
            // If no more bytes are available, we need to return as we don't
            // have more content to read.
            let n = self.buf.read(&mut buffer[carried..]).await.ok()?;
            if n == 0 {
                return None;
            }
            let filled = carried + n;

            // Only the bytes this pass actually holds may be matched: anything
            // past them is left over from an earlier window and is not part of
            // the stream at this point.
            if let Some(version) = re
                .captures(&buffer[..filled])
                .and_then(|m| m.name("version"))
                .and_then(|v| str::from_utf8(v.as_bytes()).ok())
                .map(|v| v.to_string())
            {
                // Version pattern has been found, so we need to return the
                // version.
                return Some(version);
            }

            // Keep the tail for the next pass, so a banner the read cut in half
            // is matched once its remainder arrives.
            carried = filled.min(OVERLAP);
            buffer.copy_within(filled - carried..filled, 0);
        }
    }
}

#[cfg(test)]
mod test {
    use super::{VersionFinder, WINDOW};
    use crate::{version, BinaryKind};
    use std::{
        io::Result,
        pin::Pin,
        task::{Context, Poll},
    };
    use tokio::io::{AsyncRead, AsyncSeek, ReadBuf};

    const BANNER: &[u8] =
        b"U-Boot SPL 2026.01-rc4+fslc+gd2713fb9f03+p0 (Dec 15 2025 - 18:59:28 +0000)";
    const BANNER_VERSION: &str = "2026.01-rc4+fslc+gd2713fb9f03+p0";

    /// A binary of `len` bytes carrying [`BANNER`] at `offset`.
    fn binary_with_banner_at(offset: usize, len: usize) -> Vec<u8> {
        let mut data = vec![0; len];
        data[offset..offset + BANNER.len()].copy_from_slice(BANNER);
        data
    }

    /// A reader handing out at most `chunk` bytes per read, the way a pipe or a
    /// slow device does.
    struct Trickle {
        data: Vec<u8>,
        pos: usize,
        chunk: usize,
    }

    impl AsyncRead for Trickle {
        fn poll_read(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<Result<()>> {
            let n = self
                .chunk
                .min(buf.remaining())
                .min(self.data.len() - self.pos);
            let pos = self.pos;
            buf.put_slice(&self.data[pos..pos + n]);
            self.pos += n;
            Poll::Ready(Ok(()))
        }
    }

    async fn fixture(name: &str) -> impl AsyncRead + AsyncSeek {
        use tokio::{fs::File, io::BufReader};

        BufReader::new(
            File::open(&format!("tests/fixtures/uboot/{name}"))
                .await
                .unwrap_or_else(|_| panic!("Couldn't open the fixture {name}")),
        )
    }

    /// The version must be found wherever it lands, including across the
    /// boundary between two reads. Missing it there reads as "no version at
    /// all", which silently defeats any caller comparing versions.
    #[tokio::test]
    async fn version_split_across_reads() {
        for offset in [
            0,
            1,
            WINDOW - BANNER.len() / 2,
            WINDOW - 1,
            WINDOW,
            WINDOW + 1,
        ] {
            let data = binary_with_banner_at(offset, 4 * WINDOW);
            assert_eq!(
                version(&mut std::io::Cursor::new(data), BinaryKind::UBoot).await,
                Some(BANNER_VERSION.to_string()),
                "banner at offset {offset:#x} was not found",
            );
        }
    }

    /// Short reads must not lose the banner either, nor leave bytes from an
    /// earlier window in play.
    #[tokio::test]
    async fn version_from_a_reader_with_short_reads() {
        for chunk in [1, 7, 64, 333] {
            let mut trickle = Trickle {
                data: binary_with_banner_at(WINDOW - 1, 4 * WINDOW),
                pos: 0,
                chunk,
            };
            assert_eq!(
                super::UBoot::from_reader(&mut trickle).get_version().await,
                Some(BANNER_VERSION.to_string()),
                "banner was not found reading {chunk} bytes at a time",
            );
        }
    }

    /// A binary with no banner has no version, however the reads fall.
    #[tokio::test]
    async fn no_version_in_a_binary_without_a_banner() {
        let mut trickle = Trickle {
            data: vec![0x5A; 4 * WINDOW],
            pos: 0,
            chunk: 7,
        };
        assert_eq!(
            super::UBoot::from_reader(&mut trickle).get_version().await,
            None
        );
    }

    #[tokio::test]
    async fn valid() {
        for (f, v) in &[
            ("arm-spl", "2017.11+fslc+ga07698f"),
            ("arm-u-boot-dtb.img", "2019.04-00014-gc93ced78db"),
        ] {
            assert_eq!(
                version(&mut fixture(f).await, BinaryKind::UBoot).await,
                Some(v.to_string()),
            );
        }
    }
}
