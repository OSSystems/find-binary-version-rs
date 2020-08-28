// Copyright (C) 2019 O.S. Systems Sofware LTDA
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::{format_err, Result};
use find_binary_version::{version, version_with_pattern, BinaryKind};
use std::{fs::File, io::BufReader, path::PathBuf};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "find-binary-version")]
struct Cli {
    /// Binary file to use as input
    input: PathBuf,

    /// Pattern to use to find the version
    pattern: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::from_args();

    let mut input = BufReader::new(File::open(&cli.input)?);

    let version = if let Some(pattern) = &cli.pattern {
        version_with_pattern(&mut input, pattern)
    } else {
        version(&mut input, BinaryKind::UBoot).or(version(&mut input, BinaryKind::LinuxKernel))
    };

    match version {
        Some(v) => {
            println!("{:?} has {} version", cli.input, v);
            Ok(())
        }
        None => Err(format_err!(
            "{:?} does not has a known version information.",
            cli.input
        )),
    }
}
