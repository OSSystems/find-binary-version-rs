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

#[async_trait::async_trait(?Send)]
impl<'a, R: AsyncRead + Unpin> VersionFinder for UBoot<'a, R> {
    async fn get_version(&mut self) -> Option<String> {
        // We use a fixed size buffer to avoid allocing too much memory on
        // embedded devices.
        let mut buffer = [0; 0x200];

        // Avoid recompiling the pattern.
        let re = Regex::new(r"U-Boot(?: SPL)? (?P<version>\d+.?\.[^\s]+) \(.*\)").unwrap();

        // Read the U-Boot version from the reader.
        loop {
            // If no more bytes are available, we need to return as we don't
            // have more content to read.
            let n = self.buf.read(&mut buffer).await.ok()?;
            if n == 0 {
                return None;
            }

            if let Some(version) = re
                .captures(&buffer)
                .and_then(|m| m.name("version"))
                .and_then(|v| str::from_utf8(v.as_bytes()).ok())
                .map(|v| v.to_string())
            {
                // Version pattern has been found, so we need to return the
                // version.
                return Some(version);
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
            File::open(&format!("tests/fixtures/uboot/{}", name))
                .await
                .unwrap_or_else(|_| panic!("Couldn't open the fixture {}", name)),
        )
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
