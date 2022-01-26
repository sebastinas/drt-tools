// Copyright 2022 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io::{self, BufRead, BufReader};
use std::path::PathBuf;
use std::{collections::HashSet, fs::File};

use anyhow::Result;
use structopt::StructOpt;

use crate::{config::Cache, source_packages::SourcePackages, BaseOptions};
use assorted_debian_utils::{
    architectures::RELEASE_ARCHITECTURES,
    wb::{BinNMU, SourceSpecifier, WBCommandBuilder},
};

#[derive(Debug, StructOpt)]
pub(crate) struct BinNMUBuildinfoOptions {
    /// Message for binNMUs
    #[structopt(short, long)]
    message: String,
    /// Set a build priority
    #[structopt(long = "bp")]
    build_priority: Option<i32>,
    /// Set dependency-wait
    #[structopt(long = "dw")]
    dep_wait: Option<String>,
    /// Set extra dependencies
    #[structopt(long)]
    extra_depends: Option<String>,
    /// Set the suite
    #[structopt(short, long, default_value = "unstable")]
    suite: String,
    /// Input file
    #[structopt(parse(from_os_str))]
    input: Option<PathBuf>,
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

    pub(crate) fn run(self) -> Result<()> {
        let cache = Cache::new()?;
        let mut all_paths = vec![];
        for architecture in RELEASE_ARCHITECTURES {
            all_paths.push(cache.get_cache_path(format!("Packages_{}", architecture))?);
        }
        let source_packages = SourcePackages::new(&all_paths)?;

        let matcher = regex::Regex::new("([a-z0-9+.-]+)_([^_]+)_([^-.]+)-buildd\\.buildinfo")?;

        let reader: Box<dyn BufRead> = match &self.options.input {
            None => Box::new(BufReader::new(io::stdin())),
            Some(filename) => Box::new(BufReader::new(File::open(filename)?)),
        };

        let mut wb_commands = HashSet::new();
        for line in reader.lines() {
            if line.is_err() {
                break;
            }

            let line = line.unwrap();
            if let Some(capture) = matcher.captures(&line) {
                let package = capture.get(1);
                let version = capture.get(2);
                let architecture = capture.get(3);
                if package.is_none() || version.is_none() || architecture.is_none() {
                    continue;
                }

                let package = package.unwrap().as_str();
                let mut version_split = version.unwrap().as_str().split("+b");
                let version = version_split.next().unwrap();

                let mut nmu_version = None;
                let mut source = SourceSpecifier::new(package);
                source.with_version(version).with_suite(&self.options.suite);
                if !source_packages.is_ma_same(package) {
                    source.with_archive_architectures(&[architecture
                        .unwrap()
                        .as_str()
                        .try_into()
                        .unwrap()]);
                } else {
                    if let Some(binnmu_version) = version_split.next() {
                        nmu_version = Some(binnmu_version.parse::<u32>().unwrap() + 1);
                    } else {
                        nmu_version = Some(1u32);
                    }
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
                if let Some(version) = nmu_version {
                    binnmu.with_nmu_version(version);
                }
                wb_commands.insert(binnmu.build());
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
