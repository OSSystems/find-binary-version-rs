// Copyright (C) 2019-2021 O.S. Systems Sofware LTDA
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{strings::IntoStringsIter, VersionFinder};
use regex::Regex;
use tokio::io::{AsyncRead, AsyncReadExt};

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
        // FIXME: Avoid reading the whole file
        let mut buffer = Vec::new();
        self.buf.read_to_end(&mut buffer).await.ok()?;

        let re = Regex::new(self.pattern).unwrap();
        for line in buffer.into_strings_iter() {
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
    use tokio::io::AsyncRead;

    async fn fixture(name: &str) -> impl AsyncRead {
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
                version_with_pattern(&mut fixture(f).await, r"U-Boot(?: SPL)? (\d+.?\.[^\s]+)")
                    .await,
                Some(v.to_string()),
            );
        }
    }
}
