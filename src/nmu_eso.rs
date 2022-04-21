// Copyright 2021-2022 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{collections::HashSet, io::BufRead, path::Path};

use anyhow::Result;
use clap::Parser;
use indicatif::{ProgressBar, ProgressIterator};
use serde::Deserialize;

use crate::{
    config::{self, CacheEntries, CacheState},
    BaseOptions,
};
use assorted_debian_utils::{
    architectures::{Architecture, RELEASE_ARCHITECTURES},
    archive::{Suite, SuiteOrCodename},
    wb::{BinNMU, SourceSpecifier, WBCommandBuilder},
};

#[derive(Deserialize, Debug, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
struct BinaryPackage {
    source: Option<String>,
    package: String,
    architecture: Architecture,
    #[serde(rename = "Built-Using")]
    built_using: Option<String>,
}

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
    async fn download_to_cache(&self) -> Result<CacheState> {
        self.cache.download(&[CacheEntries::Packages]).await?;
        self.cache
            .download(&[CacheEntries::OutdatedBuiltUsing])
            .await
    }

    fn parse_packages<P>(path: P) -> Result<HashSet<String>>
    where
        P: AsRef<Path>,
    {
        // read Package file
        let binary_packages: Vec<BinaryPackage> = rfc822_like::from_file(path.as_ref())?;
        let pb = ProgressBar::new(binary_packages.len() as u64);
        pb.set_style(config::default_progress_style().template(
            "{msg}: {spinner:.green} [{wide_bar:.cyan/blue}] {pos}/{len} ({per_sec}, {eta})",
        ));
        pb.set_message(format!(
            "Processing {}",
            path.as_ref().file_name().unwrap().to_str().unwrap()
        ));
        // collect all sources with arch dependendent binaries having Built-Using set
        Ok(binary_packages
            .into_iter()
            .progress_with(pb)
            .filter(|binary_package| {
                binary_package.built_using.is_some()
                    && binary_package.architecture != Architecture::All
            })
            .map(|binary_package| {
                if let Some(source_package) = &binary_package.source {
                    source_package.split_whitespace().next().unwrap().into()
                } else {
                    // no Source set, so Source == Package
                    binary_package.package
                }
            })
            .collect())
    }

    fn load_eso(&self, suite: &Suite) -> Result<HashSet<String>> {
        if self.download_to_cache()? == CacheState::NoUpdate && !self.base_options.force_processing
        {
            return Ok(HashSet::new());
        }

        let mut all_paths = vec![];
        for architecture in RELEASE_ARCHITECTURES {
            all_paths.push(
                self.cache
                    .get_cache_path(format!("Packages_{}", architecture))?,
            );
        }
        let mut actionable_sources = HashSet::<String>::new();
        for path in all_paths {
            let sources = Self::parse_packages(path);
            actionable_sources.extend(sources?);
        }

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
                Ok(ref source_suite) if source_suite == suite => {}
                _ => {
                    continue;
                }
            }

            let source = split[1].trim().to_owned();
            // not-binNMUable as the Built-Using package is binary-independent
            if !actionable_sources.contains(&source) {
                continue;
            }
            // skip some packages that either make no sense to binNMU or fail to be binNMUed
            if source.starts_with("gcc-") || source.starts_with("binutils") {
                continue;
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
