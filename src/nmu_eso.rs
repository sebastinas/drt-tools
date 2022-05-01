// Copyright 2021-2022 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::{HashMap, HashSet},
    io::BufRead,
    path::Path,
};

use anyhow::Result;
use indicatif::{ProgressBar, ProgressIterator};
use serde::Deserialize;

use crate::{
    config::{self, CacheEntries, CacheState},
    udd_bugs::load_hashmap_bugs_from_reader,
    BaseOptions, BinNMUsOptions,
};
use assorted_debian_utils::{
    architectures::{Architecture, RELEASE_ARCHITECTURES},
    archive::{Codename, Suite},
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

pub(crate) type NMUOutdatedBuiltUsingOptions = BinNMUsOptions;

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
    async fn download_to_cache(&self, codename: &Codename) -> Result<CacheState> {
        self.cache
            .download(&[
                CacheEntries::Packages,
                CacheEntries::FTBFSBugs(codename.clone()),
            ])
            .await?;
        self.cache
            .download(&[CacheEntries::OutdatedBuiltUsing])
            .await
    }

    fn load_bugs(&self, codename: &Codename) -> Result<HashMap<String, u32>> {
        load_hashmap_bugs_from_reader(
            self.cache
                .get_cache_bufreader(format!("udd-ftbfs-bugs-{}.yaml", codename))?,
        )
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
        let codename = suite.clone().into();
        if self.download_to_cache(&codename)? == CacheState::NoUpdate
            && !self.base_options.force_processing
        {
            return Ok(HashSet::new());
        }

        let ftbfs_bugs = self.load_bugs(&codename)?;
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
            // check if package FTBFS
            if let Some(bug) = ftbfs_bugs.get(&source) {
                println!("# Skipping {} due to FTBFS bug #{}", source, bug);
                continue;
            }

            result.insert(split[1].trim().to_owned());
        }

        Ok(result)
    }

    pub(crate) fn run(self) -> Result<()> {
        let suite = self.options.suite.clone().into();
        let eso_sources = self.load_eso(&suite)?;

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

            let command = binnmu.build();
            println!("{}", command);
            if !self.base_options.dry_run {
                command.execute()?;
            }
        }

        Ok(())
    }
}
