// Copyright 2021-2023 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    fs::File,
    io::{self, BufRead, BufReader},
};

use anyhow::Result;
use assorted_debian_utils::{
    package::PackageName,
    wb::{BinNMU, SourceSpecifier, WBCommandBuilder},
};
use async_trait::async_trait;
use log::{debug, warn};

use crate::{
    AsyncCommand, Downloads,
    cli::{BaseOptions, NMUListOptions},
    config::{self, CacheEntries},
    udd_bugs::UDDBugs,
    utils::execute_wb_commands,
};

pub(crate) struct NMUTransition<'a> {
    cache: &'a config::Cache,
    base_options: &'a BaseOptions,
    options: NMUListOptions,
}

impl<'a> NMUTransition<'a> {
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
}

#[async_trait]
impl AsyncCommand for NMUTransition<'_> {
    async fn run(&self) -> Result<()> {
        let ftbfs_bugs = if self.base_options.force_processing {
            UDDBugs::default()
        } else {
            UDDBugs::load_for_codename(self.cache, self.options.binnmu_options.suite)?
        };

        let mut wb_commands = Vec::new();
        {
            let reader: Box<dyn BufRead> = match &self.options.input {
                None => Box::new(BufReader::new(io::stdin())),
                Some(filename) => Box::new(BufReader::new(File::open(filename)?)),
            };

            for line in reader.lines() {
                let Ok(line) = line else {
                    break;
                };
                if line.starts_with("Dependency level") || line.is_empty() {
                    continue;
                }

                // possible formats:
                // package [build logs] (version) ...
                // package (sid only) [build logs] (version) ...
                let version_index = if line.contains("(sid only)") { 5 } else { 3 };
                let split_line: Vec<_> = line.split_whitespace().collect();
                if split_line.len() <= version_index {
                    println!("Skipping unsupported format: {line}");
                    continue;
                }

                let Ok(source) = PackageName::try_from(split_line[0]) else {
                    warn!("Invalid package name: {}", split_line[0]);
                    continue;
                };
                let version = split_line[version_index];
                let Some(version) = version.strip_prefix('(').and_then(|v| v.strip_suffix(')'))
                else {
                    warn!("Unable to parse version: {source:?} / {version:?}");
                    continue;
                };

                if let Some(bugs) = ftbfs_bugs.bugs_for_source(&source) {
                    debug!("Skipping {source} due to FTBFS bugs: {bugs:?}");
                    println!("# Skipping {source} due to FTBFS bugs");
                    continue;
                }

                let mut source = SourceSpecifier::new(&source);
                let Ok(version) = version.try_into() else {
                    warn!("Unable to parse version: {source:?} / {version:?}");
                    continue;
                };
                source
                    .with_version(&version)
                    .with_suite(self.options.binnmu_options.suite);
                if let Some(architectures) = &self.options.binnmu_options.architecture {
                    source.with_architectures(architectures);
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
                wb_commands.push(binnmu.build());
            }
        }

        execute_wb_commands(wb_commands, self.base_options).await
    }
}

impl Downloads for NMUTransition<'_> {
    fn downloads(&self) -> Vec<CacheEntries> {
        vec![CacheEntries::FTBFSBugs(self.options.binnmu_options.suite)]
    }
}
