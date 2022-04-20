// Copyright 2021-2022 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{collections::HashSet, io::BufRead};

use anyhow::Result;
use clap::Parser;

use crate::{
    config::{self, CacheEntries},
    BaseOptions,
};
use assorted_debian_utils::{
    architectures::Architecture,
    archive::{Suite, SuiteOrCodename},
    wb::{BinNMU, SourceSpecifier, WBCommandBuilder},
};

#[derive(Debug, Parser)]
pub(crate) struct NMUOutdatedBuiltUsingOptions {
    /// Message for binNMUs
    #[clap(short, long, default_value = "Rebuild for outdated Built-Using")]
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
    suite: SuiteOrCodename,
    /// Set architectures for binNMUs
    #[clap(short, long)]
    architecture: Option<Vec<Architecture>>,
}

pub(crate) struct NMUOutdatedBuiltUsing {
    cache: config::Cache,
    base_options: BaseOptions,
    options: NMUOutdatedBuiltUsingOptions,
}

impl NMUOutdatedBuiltUsing {
    pub(crate) fn new(
        base_options: BaseOptions,
        options: NMUOutdatedBuiltUsingOptions,
    ) -> Result<Self> {
        Ok(Self {
            cache: config::Cache::new(base_options.force_download)?,
            base_options,
            options,
        })
    }

    #[tokio::main]
    async fn download_to_cache(&self) -> Result<()> {
        self.cache
            .download(&[CacheEntries::OutdatedBuiltUsing])
            .await?;
        Ok(())
    }

    fn load_eso(&self, suite: &Suite) -> Result<HashSet<String>> {
        self.download_to_cache()?;

        let mut result = HashSet::new();
        let reader = self.cache.get_cache_bufreader("outdated-built-using.txt")?;

        for line in reader.lines() {
            if line.is_err() {
                break;
            }

            let line = line.unwrap();
            let split: Vec<&str> = line.split(" | ").collect();
            if split.len() != 5 {
                continue;
            }

            // check if suite matches
            match Suite::try_from(split[0].trim()) {
                Ok(ref source_suite) => {
                    if source_suite != suite {
                        continue;
                    }
                }
                _ => {
                    continue;
                }
            }

            result.insert(split[1].trim().to_owned());
        }

        Ok(result)
    }

    pub(crate) fn run(self) -> Result<()> {
        let suite = self.options.suite.clone().into();
        let eso_sources = self.load_eso(&suite)?;

        let mut wb_commands = Vec::new();
        for source in eso_sources {
            let mut source = SourceSpecifier::new(&source);
            source.with_suite(&self.options.suite);
            if let Some(architectures) = &self.options.architecture {
                source.with_archive_architectures(architectures);
            }

            let mut binnmu = BinNMU::new(&source, &self.options.message)?;
            if let Some(bp) = self.options.build_priority {
                binnmu.with_build_priority(bp);
            }
            if let Some(dw) = &self.options.dep_wait {
                binnmu.with_dependency_wait(dw);
            }
            if let Some(extra_depends) = &self.options.extra_depends {
                binnmu.with_extra_depends(extra_depends);
            }
            wb_commands.push(binnmu.build())
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
