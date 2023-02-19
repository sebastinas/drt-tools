// Copyright 2021-2022 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::HashSet,
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

use anyhow::Result;
use assorted_debian_utils::{
    architectures::Architecture,
    archive::{Codename, Suite, SuiteOrCodename},
    rfc822_like,
    wb::{BinNMU, SourceSpecifier, WBCommandBuilder},
};
use clap::Parser;
use indicatif::{ProgressBar, ProgressIterator};
use itertools::sorted;
use log::{debug, trace};
use serde::Deserialize;

use crate::{
    config::{default_progress_style, Cache, CacheEntries, CacheState},
    udd_bugs::{load_bugs_from_reader, UDDBugs},
    BaseOptions,
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
    /// Build priority. If specified, the binNMUs are scheduled with the given build priority. Builds with a positive priority will be built earlier.
    #[clap(long = "bp", default_value = "-50")]
    build_priority: i32,
    /// Suite for binNMUs.
    #[clap(short, long, default_value = "unstable")]
    suite: SuiteOrCodename,
    /// Set architectures for binNMUs. If no archictures are specified, the binNMUs are scheduled with ANY.
    #[clap(short, long)]
    architecture: Option<Vec<Architecture>>,
}

pub(crate) struct NMUOutdatedBuiltUsing {
    cache: Cache,
    base_options: BaseOptions,
    options: NMUOutdatedBuiltUsingOptions,
}

struct PackageReader {
    reader: BufReader<File>,
    suite: Suite,
    actionable_sources: HashSet<String>,
    ftbfs_bugs: UDDBugs,
}

impl PackageReader {
    fn new(
        reader: BufReader<File>,
        suite: Suite,
        actionable_sources: HashSet<String>,
        ftbfs_bugs: UDDBugs,
    ) -> Self {
        Self {
            reader,
            suite,
            actionable_sources,
            ftbfs_bugs,
        }
    }
}

impl Iterator for PackageReader {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        let mut line = String::new();
        while let Ok(size) = {
            line.clear();
            self.reader.read_line(&mut line)
        } {
            if size == 0 {
                break;
            }

            trace!("Processing line: {}", line);
            let split: Vec<&str> = line.split(" | ").collect();
            if split.len() != 5 {
                continue;
            }

            // check if suite matches
            match Suite::try_from(split[0].trim()) {
                Ok(source_suite) if source_suite == self.suite => {}
                _ => {
                    continue;
                }
            }

            let source = split[1].trim().to_owned();
            // not-binNMUable as the Built-Using package is binary-independent
            if !self.actionable_sources.contains(&source) {
                debug!("Skipping {}: not actionable", source);
                continue;
            }
            // skip some packages that either make no sense to binNMU or fail to be binNMUed
            if source.starts_with("gcc-") || source.starts_with("binutils") {
                debug!("Skipping {}: either gcc or binuitls", source);
                continue;
            }
            // skip grub/linux/... signed packages
            if source.ends_with("-signed")
                && (source.starts_with("grub-")
                    || source.starts_with("linux-")
                    || source.starts_with("shim-")
                    || source.starts_with("fwupd-"))
            {
                debug!("Skipping {}: signed package", source);
                continue;
            }

            // check if package FTBFS
            if let Some(bugs) = self.ftbfs_bugs.bugs_for_source(&source) {
                println!("# Skipping {} due to FTBFS bugs ...", source);
                for bug in bugs {
                    debug!(
                        "Skipping {}: #{} - {}: {}",
                        source, bug.id, bug.severity, bug.title
                    );
                }
                continue;
            }

            return Some(source);
        }

        None
    }
}

impl NMUOutdatedBuiltUsing {
    pub(crate) fn new(
        base_options: BaseOptions,
        options: NMUOutdatedBuiltUsingOptions,
    ) -> Result<Self> {
        Ok(Self {
            cache: Cache::new(base_options.force_download, &base_options.mirror)?,
            base_options,
            options,
        })
    }

    async fn download_to_cache(&self, codename: Codename) -> Result<CacheState> {
        self.cache
            .download(&[CacheEntries::Packages, CacheEntries::FTBFSBugs(codename)])
            .await?;
        self.cache
            .download(&[CacheEntries::OutdatedBuiltUsing])
            .await
    }

    fn load_bugs(&self, codename: Codename) -> Result<UDDBugs> {
        load_bugs_from_reader(
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
        pb.set_style(default_progress_style().template(
            "{msg}: {spinner:.green} [{wide_bar:.cyan/blue}] {pos}/{len} ({per_sec}, {eta})",
        )?);
        pb.set_message(format!("Processing {}", path.as_ref().display()));
        // collect all sources with arch dependent binaries having Built-Using set
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

    async fn load_eso(&self, suite: Suite) -> Result<Vec<String>> {
        let codename = suite.into();
        if self.download_to_cache(codename).await? == CacheState::NoUpdate
            && !self.base_options.force_processing
        {
            return Ok(Vec::new());
        }

        let ftbfs_bugs = self.load_bugs(codename)?;
        let mut actionable_sources = HashSet::<String>::new();
        for path in self.cache.get_package_paths(false)? {
            let sources = Self::parse_packages(path);
            actionable_sources.extend(sources?);
        }

        let result: HashSet<String> = PackageReader::new(
            self.cache.get_cache_bufreader("outdated-built-using.txt")?,
            suite,
            actionable_sources,
            ftbfs_bugs,
        )
        .into_iter()
        .collect();

        Ok(sorted(result.into_iter()).collect())
    }

    pub(crate) async fn run(self) -> Result<()> {
        let suite = self.options.suite.into();
        let eso_sources = self.load_eso(suite).await?;

        for source in eso_sources {
            let mut source = SourceSpecifier::new(&source);
            source.with_suite(&self.options.suite);
            if let Some(architectures) = &self.options.architecture {
                source.with_archive_architectures(architectures);
            }

            let mut binnmu = BinNMU::new(&source, "Rebuild for outdated Built-Using")?;
            binnmu.with_build_priority(self.options.build_priority);

            let command = binnmu.build();
            println!("{}", command);
            if !self.base_options.dry_run {
                command.execute()?;
            }
        }

        Ok(())
    }
}
