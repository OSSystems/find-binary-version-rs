// Copyright (C) 2019 O.S. Systems Sofware LTDA
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use exitfailure::ExitFailure;
use failure::format_err;
use find_binary_version::{version, BinaryKind};
use std::{fs::File, io::BufReader, path::PathBuf};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "find-binary-version")]
struct Cli {
    /// Binary file to use as input
    input: PathBuf,
}

fn main() -> Result<(), ExitFailure> {
    let cli = Cli::from_args();

    let mut input = BufReader::new(File::open(&cli.input)?);

    for kind in &[BinaryKind::UBoot, BinaryKind::LinuxKernel] {
        if let Some(version) = version(*kind, &mut input) {
            println!("{}", version);
            return Ok(());
        }
    }

    Err(format_err!("{:?} does not has a known version information.", cli.input).into())
}
