// Copyright 2021-2022 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::PathBuf;

use anyhow::Result;
use assorted_debian_utils::{
    archive::Codename,
    wb::{BinNMU, SourceSpecifier, WBCommandBuilder},
};
use clap::Parser;

use crate::{
    config::{self, CacheEntries},
    udd_bugs::{load_bugs_from_reader, UDDBugs},
    BaseOptions, BinNMUsOptions,
};

#[derive(Debug, Parser)]
pub(crate) struct PrepareBinNMUsOptions {
    #[clap(flatten)]
    binnmu_options: BinNMUsOptions,
    /// Input file with a list of packages. If not specified, the list of packages will be read from he standard input.
    #[clap(parse(from_os_str))]
    input: Option<PathBuf>,
}

pub(crate) struct PrepareBinNMUs {
    cache: config::Cache,
    base_options: BaseOptions,
    options: PrepareBinNMUsOptions,
}

impl PrepareBinNMUs {
    pub(crate) fn new(base_options: BaseOptions, options: PrepareBinNMUsOptions) -> Result<Self> {
        Ok(Self {
            cache: config::Cache::new(base_options.force_download)?,
            base_options,
            options,
        })
    }

    #[tokio::main]
    async fn download_to_cache(&self, codename: &Codename) -> Result<()> {
        self.cache
            .download(&[CacheEntries::FTBFSBugs(*codename)])
            .await?;
        Ok(())
    }

    fn load_bugs(&self, codename: &Codename) -> Result<UDDBugs> {
        self.download_to_cache(codename)?;
        load_bugs_from_reader(
            self.cache
                .get_cache_bufreader(format!("udd-ftbfs-bugs-{}.yaml", codename))?,
        )
    }

    pub(crate) fn run(self) -> Result<()> {
        let codename: Codename = self.options.binnmu_options.suite.into();
        let ftbfs_bugs = if !self.base_options.force_processing {
            self.load_bugs(&codename)?
        } else {
            UDDBugs::new(vec![])
        };

        let matcher = regex::Regex::new("([a-z0-9+.-]+)[ \t].* \\(?([0-9][^() \t]*)\\)?")?;

        let reader: Box<dyn BufRead> = match &self.options.input {
            None => Box::new(BufReader::new(io::stdin())),
            Some(filename) => Box::new(BufReader::new(File::open(filename)?)),
        };

        let mut wb_commands = Vec::new();
        for line in reader.lines() {
            let line = match line {
                Ok(line) => line,
                Err(_) => break,
            };
            if line.starts_with("Dependency level") {
                continue;
            }

            if let Some(capture) = matcher.captures(&line) {
                let package = capture.get(1);
                let version = capture.get(2);

                let (source, version) = match (package, version) {
                    (Some(package), Some(version)) => (package.as_str(), version.as_str()),
                    _ => continue,
                };

                if let Some(bugs) = ftbfs_bugs.bugs_for_source(source) {
                    println!("# Skipping {} due to FTBFS bugs ...", source);
                    for bug in bugs {
                        println!("#   {} ({}): {}", bug.id, bug.severity, bug.title);
                    }
                    continue;
                }

                let mut source = SourceSpecifier::new(source);
                let version = version.try_into()?;
                source
                    .with_version(&version)
                    .with_suite(&self.options.binnmu_options.suite);
                if let Some(architectures) = &self.options.binnmu_options.architecture {
                    source.with_archive_architectures(architectures);
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
                wb_commands.push(binnmu.build())
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
