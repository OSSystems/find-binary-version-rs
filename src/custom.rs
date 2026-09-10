// Copyright (C) 2019-2021 O.S. Systems Sofware LTDA
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{strings::Strings, VersionFinder};
use regex::Regex;
use tokio::io::AsyncRead;

pub(crate) struct Custom<'a, R>
where
    R: AsyncRead + Unpin,
{
    buf: &'a mut R,
    pattern: &'a str,
}

impl<'a, R> Custom<'a, R>
where
    R: AsyncRead + Unpin,
{
    pub(crate) fn from_reader(buf: &'a mut R, pattern: &'a str) -> Self {
        Custom { buf, pattern }
    }
}

#[async_trait::async_trait(?Send)]
impl<'a, R: AsyncRead + Unpin> VersionFinder for Custom<'a, R> {
    async fn get_version(&mut self) -> Option<String> {
        let re = Regex::new(self.pattern).unwrap();
        let mut strings = Strings::from_reader(&mut *self.buf);

        while let Some(line) = strings.next_string().await {
            if let Some(v) = re.captures(&line).and_then(|c| c.get(1)) {
                return Some(v.as_str().to_string());
            }
        }

        None
    }
}

#[cfg(test)]
mod test {
    use crate::version_with_pattern;
    use std::io::Cursor;
    use tokio::io::AsyncRead;

    const PATTERN: &str = r"U-Boot(?: SPL)? (\d+.?\.[^\s]+)";

    async fn fixture(name: &str) -> impl AsyncRead {
        use tokio::{fs::File, io::BufReader};

        BufReader::new(
            File::open(&format!("tests/fixtures/uboot/{name}"))
                .await
                .unwrap_or_else(|_| panic!("Couldn't open the fixture {name}")),
        )
    }

    /// The search must stop at the version rather than walking the rest of the
    /// source, which on a device is an image far too large to hold in memory.
    #[tokio::test]
    async fn stops_reading_once_the_version_is_found() {
        let mut data =
            b"\0U-Boot SPL 2017.11+fslc+ga07698f (Jan 1 2020 - 00:00:00 +0000)\0".to_vec();
        data.resize(64 * 1024, 0);
        let len = data.len() as u64;

        let mut source = Cursor::new(data);
        assert_eq!(
            version_with_pattern(&mut source, PATTERN).await,
            Some("2017.11+fslc+ga07698f".to_string()),
        );
        assert!(
            source.position() < len,
            "read {} of {len} bytes, so the whole source was consumed",
            source.position(),
        );
    }

    #[tokio::test]
    async fn valid() {
        for (f, v) in &[
            ("arm-spl", "2017.11+fslc+ga07698f"),
            ("arm-u-boot-dtb.img", "2019.04-00014-gc93ced78db"),
        ] {
            assert_eq!(
                version_with_pattern(&mut fixture(f).await, PATTERN).await,
                Some(v.to_string()),
            );
        }
    }
}
