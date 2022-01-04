// Copyright (C) 2019-2021 O.S. Systems Sofware LTDA
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::{format_err, Result};
use find_binary_version::{version, version_with_pattern, BinaryKind};
use std::path::PathBuf;
use structopt::StructOpt;
use tokio::{fs::File, io::BufReader};

#[derive(StructOpt, Debug)]
#[structopt(name = "find-binary-version")]
struct Cli {
    /// Binary file to use as input
    input: PathBuf,

    /// Pattern to use to find the version
    pattern: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::from_args();

    let mut input = BufReader::new(File::open(&cli.input).await?);

    let version = if let Some(pattern) = &cli.pattern {
        version_with_pattern(&mut input, pattern).await
    } else {
        version(&mut input, BinaryKind::UBoot).await.or(version(
            &mut input,
            BinaryKind::LinuxKernel,
        )
        .await)
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
