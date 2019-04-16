// Copyright (C) 2019 O.S. Systems Sofware LTDA
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{strings::IntoStringsIter, VersionFinder};
use regex::Regex;
use std::io::Read;

pub(crate) struct UBoot<'a, R> {
    buf: &'a mut R,
}

impl<'a, R> UBoot<'a, R> {
    pub(crate) fn from_reader(buf: &'a mut R) -> Self {
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

#[test]
fn u_boot() {
    use crate::{version, BinaryKind};
    use std::io::Cursor;

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
