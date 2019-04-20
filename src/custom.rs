// Copyright (C) 2019 O.S. Systems Sofware LTDA
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{strings::IntoStringsIter, VersionFinder};
use regex::Regex;
use std::io::Read;

pub(crate) struct Custom<'a, R> {
    buf: &'a mut R,
    pattern: &'a str,
}

impl<'a, R> Custom<'a, R> {
    pub(crate) fn from_reader(buf: &'a mut R, pattern: &'a str) -> Self {
        Custom { buf, pattern }
    }
}

impl<'a, R: Read> VersionFinder for Custom<'a, R> {
    fn get_version(&mut self) -> Option<String> {
        // FIXME: Avoid reading the whole file
        let mut buffer = Vec::new();
        self.buf.read_to_end(&mut buffer).ok()?;

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
    use std::io::{Read, Seek};

    fn fixture(name: &str) -> impl Read + Seek {
        use std::{fs::File, io::BufReader};

        BufReader::new(
            File::open(&format!("tests/fixtures/uboot/{}", name))
                .unwrap_or_else(|_| panic!("Couldn't open the fixture {}", name)),
        )
    }

    #[test]
    fn valid() {
        for (f, v) in &[
            ("arm-spl", "2019.04-00014-gc93ced78db"),
            ("arm-u-boot-dtb.img", "2019.04-00014-gc93ced78db"),
        ] {
            assert_eq!(
                version_with_pattern(&mut fixture(f), r"U-Boot(?: SPL)? (\d+.?\.[^\s]+)"),
                Some(v.to_string()),
            );
        }
    }
}
