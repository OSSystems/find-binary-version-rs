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

mod linuxkernel;
mod strings;
mod uboot;

use crate::{linuxkernel::LinuxKernel, uboot::UBoot};
use std::io::{Read, Seek};

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
