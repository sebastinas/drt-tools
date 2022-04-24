// Copyright 2022 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::{collections::HashSet, fs::File};

use anyhow::{anyhow, Result};
use clap::Parser;

use crate::{
    config::{Cache, CacheEntries, CacheState},
    source_packages::SourcePackages,
    BaseOptions, BinNMUsOptions,
};
use assorted_debian_utils::{
    architectures::{Architecture, RELEASE_ARCHITECTURES},
    buildinfo::{self, Buildinfo},
    wb::{BinNMU, SourceSpecifier, WBCommand, WBCommandBuilder},
};

#[derive(Debug, Parser)]
pub(crate) struct BinNMUBuildinfoOptions {
    #[clap(flatten)]
    binnmu_options: BinNMUsOptions,
    /// Input files
    #[clap(parse(from_os_str))]
    inputs: Vec<PathBuf>,
}

pub(crate) struct BinNMUBuildinfo {
    cache: Cache,
    base_options: BaseOptions,
    options: BinNMUBuildinfoOptions,
}

impl BinNMUBuildinfo {
    pub(crate) fn new(base_options: BaseOptions, options: BinNMUBuildinfoOptions) -> Result<Self> {
        Ok(Self {
            cache: Cache::new(base_options.force_download)?,
            base_options,
            options,
        })
    }

    #[tokio::main]
    async fn download_to_cache(&self) -> Result<CacheState> {
        self.cache.download(&[CacheEntries::Packages]).await?;
        Ok(CacheState::FreshFiles)
    }

    fn process(&self, buildinfo: Buildinfo, source_packages: &SourcePackages) -> Result<WBCommand> {
        let mut source_split = buildinfo.source.split_whitespace();
        let source_package = source_split.next().unwrap();

        let architectures: Vec<Architecture> = buildinfo
            .architecture
            .into_iter()
            .filter(|arch| *arch == Architecture::All || *arch == Architecture::Source)
            .collect();
        if architectures.is_empty() {
            return Err(anyhow!("no binNMU-able architecture"));
        }

        // let mut nmu_version = None;
        let mut source = SourceSpecifier::new(source_package);
        let version = buildinfo.version.without_binnmu_version();
        source
            .with_version(&version)
            .with_suite(&self.options.binnmu_options.suite);
        if !source_packages.is_ma_same(source_package) {
            // binNMU only on the architecture if no MA: same binary packages
            source.with_archive_architectures(&architectures);
            //  } else {
            //      if let Some(binnmu_version) = version_split.next() {
            //          nmu_version = Some(binnmu_version.parse::<u32>().unwrap() + 1);
            //      } else {
            //          nmu_version = Some(1u32);
            //      }
        }

        let mut binnmu = BinNMU::new(&source, &self.options.binnmu_options.message)?;
        if let Some(bp) = self.options.binnmu_options.build_priority {
            binnmu.with_build_priority(bp);
        }
        if let Some(dw) = &self.options.binnmu_options.dep_wait {
            binnmu.with_dependency_wait(dw);
        }
        if let Some(extra_depends) = &self.options.binnmu_options.extra_depends {
            binnmu.with_extra_depends(extra_depends);
        }
        //  if let Some(version) = nmu_version {
        //      binnmu.with_nmu_version(version);
        //  }
        Ok(binnmu.build())
    }

    pub(crate) fn run(self) -> Result<()> {
        self.download_to_cache()?;

        let mut all_paths = vec![];
        for architecture in RELEASE_ARCHITECTURES {
            all_paths.push(
                self.cache
                    .get_cache_path(format!("Packages_{}", architecture))?,
            );
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
                    let command = self.process(bi, &source_packages);
                    if command.is_err() {
                        println!(
                            "# skipping {}: {}",
                            filename.display(),
                            command.unwrap_err()
                        );
                        continue;
                    }
                    wb_commands.insert(command.unwrap());
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
