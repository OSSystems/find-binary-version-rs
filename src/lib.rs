// Copyright (C) 2019-2021 O.S. Systems Software LTDA
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

//! The library provide a way for reading version from the binaries files.
//!
//! ---
//!
//! ## Dependencies
//!
//! You must have `libarchive` properly installed on your system in order to use
//! this. If building on *nix systems, `pkg-config` is used to locate the
//! `libarchive`; on Windows `vcpkg` will be used to locating the `libarchive`.
//!
//! The minimum supported Rust version is 1.59.
//!
//! ## Features
//!
//! The following know patterns are supported allowing the version to be detected
//! without the need for any user specification:
//!
//! * U-Boot
//! * LinuxKernel
//!
//! Other formats are supported through the `version_with_pattern` function,
//! which will look for a given regular expression on the given binary.

mod custom;
mod linuxkernel;
mod strings;
mod uboot;

use crate::{custom::Custom, linuxkernel::LinuxKernel, uboot::UBoot};
use tokio::io::{AsyncRead, AsyncSeek};

#[derive(Debug, Copy, Clone)]
/// Define the binary kind to use for matching.
pub enum BinaryKind {
    /// U-Boot binary kind.
    UBoot,
    /// Linux Kernel binary kind.
    LinuxKernel,
}

#[async_trait::async_trait(?Send)]
trait VersionFinder {
    async fn get_version(&mut self) -> Option<String>;
}

/// Get the version for a specific binary.
pub async fn version<R: AsyncRead + AsyncSeek + Unpin>(
    mut buffer: &mut R,
    kind: BinaryKind,
) -> Option<String> {
    match kind {
        BinaryKind::LinuxKernel => LinuxKernel::from_reader(&mut buffer).get_version().await,
        BinaryKind::UBoot => UBoot::from_reader(&mut buffer).get_version().await,
    }
}

/// Get the version for a specific pattern.
pub async fn version_with_pattern<R: AsyncRead + Unpin>(
    mut buffer: &mut R,
    pattern: &str,
) -> Option<String> {
    Custom::from_reader(&mut buffer, pattern)
        .get_version()
        .await
}
