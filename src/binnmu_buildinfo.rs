// Copyright 2022 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::{collections::HashSet, fs::File};

use anyhow::Result;
use clap::Parser;

use crate::{config::Cache, source_packages::SourcePackages, BaseOptions};
use assorted_debian_utils::{
    architectures::{Architecture, RELEASE_ARCHITECTURES},
    buildinfo::{self, Buildinfo},
    wb::{BinNMU, SourceSpecifier, WBCommand, WBCommandBuilder},
};

#[derive(Debug, Parser)]
pub(crate) struct BinNMUBuildinfoOptions {
    /// Message for binNMUs
    #[clap(short, long)]
    message: String,
    /// Set a build priority
    #[clap(long = "bp")]
    build_priority: Option<i32>,
    /// Set dependency-wait
    #[clap(long = "dw")]
    dep_wait: Option<String>,
    /// Set extra dependencies
    #[clap(long)]
    extra_depends: Option<String>,
    /// Set the suite
    #[clap(short, long, default_value = "unstable")]
    suite: String,
    /// Input files
    #[clap(parse(from_os_str))]
    inputs: Vec<PathBuf>,
}

pub(crate) struct BinNMUBuildinfo {
    base_options: BaseOptions,
    options: BinNMUBuildinfoOptions,
}

impl BinNMUBuildinfo {
    pub(crate) fn new(base_options: BaseOptions, options: BinNMUBuildinfoOptions) -> Self {
        Self {
            base_options,
            options,
        }
    }

    fn process(&self, buildinfo: Buildinfo, source_packages: &SourcePackages) -> WBCommand {
        let mut source_split = buildinfo.source.split_whitespace();
        let source_package = source_split.next().unwrap();

        let mut version_split = buildinfo.version.split("+b");
        let version = version_split.next().unwrap();

        // let mut nmu_version = None;
        let mut source = SourceSpecifier::new(source_package);
        source.with_version(version).with_suite(&self.options.suite);
        if !source_packages.is_ma_same(source_package) {
            // binNMU only on the architecture if no MA: same binary packages
            source.with_archive_architectures(&[buildinfo.architecture]);
            //  } else {
            //      if let Some(binnmu_version) = version_split.next() {
            //          nmu_version = Some(binnmu_version.parse::<u32>().unwrap() + 1);
            //      } else {
            //          nmu_version = Some(1u32);
            //      }
        }

        let mut binnmu = BinNMU::new(&source, &self.options.message);
        if let Some(bp) = self.options.build_priority {
            binnmu.with_build_priority(bp);
        }
        if let Some(dw) = &self.options.dep_wait {
            binnmu.with_dependency_wait(dw);
        }
        if let Some(extra_depends) = &self.options.extra_depends {
            binnmu.with_extra_depends(extra_depends);
        }
        //  if let Some(version) = nmu_version {
        //      binnmu.with_nmu_version(version);
        //  }
        binnmu.build()
    }

    pub(crate) fn run(self) -> Result<()> {
        let cache = Cache::new()?;
        let mut all_paths = vec![];
        for architecture in RELEASE_ARCHITECTURES {
            all_paths.push(cache.get_cache_path(format!("Packages_{}", architecture))?);
        }
        let source_packages = SourcePackages::new(&all_paths)?;

        let mut wb_commands = HashSet::new();
        // iterate over all buildinfo files
        for filename in &self.options.inputs {
            let data = strip_signature(BufReader::new(File::open(&filename)?))?;
            match buildinfo::from_reader(data.as_ref()) {
                Err(e) => {
                    println!("# skipping {}: {}", filename.display(), e);
                    continue;
                }
                Ok(bi) => {
                    if bi.architecture == Architecture::All {
                        println!(
                            "# skipping {}: architecture all not binnumable",
                            filename.display()
                        )
                    }
                    wb_commands.insert(self.process(bi, &source_packages));
                }
            }
        }

        for commands in wb_commands {
            println!("{}", commands);
            if !self.base_options.dry_run {
                commands.execute()?;
            }
        }

        Ok(())
    }
}

// Strip the signature from a buildinfo file without verifying it
fn strip_signature(input: impl BufRead) -> Result<Vec<u8>> {
    let mut data = vec![];
    for line in input.lines().skip_while(|rline| {
        if let Ok(line) = rline {
            // Skip until the beginning of a buildinfo file
            !line.starts_with("Format: ")
        } else {
            true
        }
    }) {
        if line.is_err() {
            break;
        }

        let line = line.unwrap();
        // Read until beginning of the signature block
        if line.starts_with("-----BEGIN") {
            return Ok(data);
        }
        data.write_all(line.as_bytes())?;
        data.write_all(b"\n")?;
    }

    Ok(data)
}
