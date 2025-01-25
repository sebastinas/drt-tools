// Copyright 2024 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    fs::File,
    io::{self, BufRead, BufReader},
};

use anyhow::Result;
use assorted_debian_utils::{
    archive::Codename,
    version::PackageVersion,
    wb::{BinNMU, SourceSpecifier, WBCommandBuilder},
};
use async_trait::async_trait;
use log::debug;

use crate::{
    cli::{BaseOptions, NMUListOptions},
    config::{self, CacheEntries},
    source_packages::SourcePackages,
    udd_bugs::{load_bugs_from_reader, UDDBugs},
    utils::execute_wb_commands,
    AsyncCommand, Downloads,
};

pub(crate) struct NMUList<'a> {
    cache: &'a config::Cache,
    base_options: &'a BaseOptions,
    options: NMUListOptions,
}

impl<'a> NMUList<'a> {
    pub(crate) fn new(
        cache: &'a config::Cache,
        base_options: &'a BaseOptions,
        options: NMUListOptions,
    ) -> Self {
        Self {
            cache,
            base_options,
            options,
        }
    }

    fn load_bugs(&self, codename: Codename) -> Result<UDDBugs> {
        load_bugs_from_reader(
            self.cache
                .get_cache_bufreader(format!("udd-ftbfs-bugs-{codename}.yaml"))?,
        )
    }
}

#[async_trait]
impl AsyncCommand for NMUList<'_> {
    async fn run(&self) -> Result<()> {
        let codename: Codename = self.options.binnmu_options.suite.into();
        let source_packages = SourcePackages::new(
            &self
                .cache
                .get_package_paths(self.options.binnmu_options.suite.into(), false)?,
        )?;
        let ftbfs_bugs = if self.base_options.force_processing {
            UDDBugs::new(vec![])
        } else {
            self.load_bugs(codename)?
        };

        let mut wb_commands = Vec::new();
        let reader: Box<dyn BufRead> = match &self.options.input {
            None => Box::new(BufReader::new(io::stdin())),
            Some(filename) => Box::new(BufReader::new(File::open(filename)?)),
        };
        for line in reader.lines() {
            let Ok(line) = line else {
                // EOF
                break;
            };

            for source in line.split_whitespace() {
                if source.is_empty() {
                    // should never happen
                    continue;
                }

                let (source, version) = source
                    .split_once('_')
                    .map(|(source, version)| (source, PackageVersion::try_from(version).ok()))
                    .unwrap_or_else(|| (source, source_packages.version(source).cloned()));
                if source.is_empty() {
                    continue;
                }

                if let Some(bugs) = ftbfs_bugs.bugs_for_source(source) {
                    debug!("Skipping {} due to FTBFS bugs: {:?}", source, bugs);
                    println!("# Skipping {source} due to FTBFS bugs");
                    continue;
                }

                let mut source_specifier = SourceSpecifier::new(source);
                if let Some(ref version) = version {
                    source_specifier.with_version(version);
                }
                source_specifier.with_suite(self.options.binnmu_options.suite);
                if let Some(architectures) = &self.options.binnmu_options.architecture {
                    if !source_packages.is_ma_same(source) {
                        source_specifier.with_architectures(architectures);
                    }
                }

                let mut binnmu =
                    BinNMU::new(&source_specifier, &self.options.binnmu_options.message)?;
                if let Some(bp) = self.options.binnmu_options.build_priority {
                    binnmu.with_build_priority(bp);
                }
                if let Some(dw) = &self.options.binnmu_options.dep_wait {
                    binnmu.with_dependency_wait(dw);
                }
                if let Some(extra_depends) = &self.options.binnmu_options.extra_depends {
                    binnmu.with_extra_depends(extra_depends);
                }
                wb_commands.push(binnmu.build());
            }
        }

        execute_wb_commands(wb_commands, self.base_options).await
    }
}

impl Downloads for NMUList<'_> {
    fn downloads(&self) -> Vec<CacheEntries> {
        vec![CacheEntries::FTBFSBugs(self.options.binnmu_options.suite)]
    }
}
