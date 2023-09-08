// Copyright 2023 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::{HashMap, HashSet},
    iter::FusedIterator,
    path::Path,
    vec::IntoIter,
};

use anyhow::Result;
use assorted_debian_utils::{
    architectures::Architecture,
    archive::{Codename, MultiArch, Suite, SuiteOrCodename},
    rfc822_like,
    version::PackageVersion,
    wb::{BinNMU, SourceSpecifier, WBCommandBuilder},
};
use async_trait::async_trait;
use clap::Parser;
use indicatif::{ProgressBar, ProgressBarIter, ProgressIterator};
use itertools::Itertools;
use log::{debug, error};
use serde::Deserialize;

use crate::{
    config::{default_progress_style, source_skip_binnmu, Cache, CacheEntries},
    udd_bugs::{load_bugs_from_reader, UDDBugs},
    BaseOptions, Command,
};

#[derive(Deserialize, Debug, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
struct BinaryPackage {
    source: Option<String>,
    package: String,
    version: PackageVersion,
    architecture: Architecture,
    #[serde(rename = "Multi-Arch")]
    multi_arch: Option<MultiArch>,
}

#[derive(Debug, Parser)]
pub(crate) struct NMUVersionSkewOptions {
    /// Build priority. If specified, the binNMUs are scheduled with the given build priority. Builds with a positive priority will be built earlier.
    #[clap(long = "bp", default_value = "-50")]
    build_priority: i32,
    /// Suite for binNMUs.
    #[clap(short, long, default_value = "unstable")]
    suite: SuiteOrCodename,
}

struct BinaryPackageParser {
    iterator: ProgressBarIter<IntoIter<BinaryPackage>>,
}

impl BinaryPackageParser {
    fn new<P>(path: P) -> Result<Self>
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
        // collect all sources with arch dependent binaries having Built-Using set and their Built-Using fields
        Ok(Self {
            iterator: binary_packages.into_iter().progress_with(pb),
        })
    }
}

impl Iterator for BinaryPackageParser {
    type Item = (String, Architecture, PackageVersion);

    fn next(&mut self) -> Option<Self::Item> {
        for binary_package in self.iterator.by_ref() {
            // skip packages that are not MA: same
            if binary_package.multi_arch != Some(MultiArch::Same) {
                continue;
            }
            // skip Arch: all packages
            if binary_package.architecture == Architecture::All {
                continue;
            }

            return Some((
                if let Some(source_package) = &binary_package.source {
                    match source_package.split_whitespace().next() {
                        Some(package) => package.into(),
                        None => continue,
                    }
                } else {
                    // no Source set, so Source == Package
                    binary_package.package
                },
                binary_package.architecture,
                binary_package.version,
            ));
        }

        None
    }
}

impl FusedIterator for BinaryPackageParser {}

pub(crate) struct NMUVersionSkew<'a> {
    cache: &'a Cache,
    base_options: &'a BaseOptions,
    options: NMUVersionSkewOptions,
}

impl<'a> NMUVersionSkew<'a> {
    pub(crate) fn new(
        cache: &'a Cache,
        base_options: &'a BaseOptions,
        options: NMUVersionSkewOptions,
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
                .get_cache_bufreader(format!("udd-ftbfs-bugs-{}.yaml", codename))?,
        )
    }

    fn load_version_skew(
        &self,
        suite: Suite,
    ) -> Result<Vec<(String, PackageVersion, Vec<Architecture>)>> {
        let codename = suite.into();
        let ftbfs_bugs = self.load_bugs(codename)?;
        let mut packages: HashMap<String, HashSet<(Architecture, PackageVersion)>> = HashMap::new();
        for path in self.cache.get_package_paths(suite, false)? {
            for (source, architecture, version) in BinaryPackageParser::new(path)? {
                // skip some packages that make no sense to binNMU
                if source_skip_binnmu(&source) {
                    continue;
                }

                if let Some(info) = packages.get_mut(&source) {
                    info.insert((architecture, version));
                } else {
                    packages.insert(source, {
                        let mut hs = HashSet::new();
                        hs.insert((architecture, version));
                        hs
                    });
                }
            }
        }

        let mut sources_architectures =
            Vec::<(String, PackageVersion, Vec<Architecture>)>::default();
        for (source, source_info) in packages.into_iter().sorted_by_key(|value| value.0.clone()) {
            let all_versions: HashSet<&PackageVersion> =
                HashSet::from_iter(source_info.iter().map(|(_, version)| version));
            if all_versions.len() == 1 {
                debug!("Skipping {}: package is in sync", source);
                continue;
            }

            let all_versions_without_binnmu: HashSet<PackageVersion> = HashSet::from_iter(
                source_info
                    .iter()
                    .map(|(_, version)| version.clone().without_binnmu_version()),
            );
            if all_versions_without_binnmu.len() != 1 {
                debug!(
                    "Skipping {}: package is out-of-date on some architecture",
                    source
                );
                continue;
            }
            // check if package FTBFS
            if let Some(bugs) = ftbfs_bugs.bugs_for_source(&source) {
                println!("# Skipping {} due to FTBFS bugs ...", source);
                for bug in bugs {
                    debug!(
                        "Skipping {}: #{} - {}: {}",
                        source, bug.id, bug.severity, bug.title
                    );
                }
                continue;
            }

            let Some(max_version) = all_versions.iter().max().map(|v| (*v).clone()) else {
                error!(
                    "Skipping {}: package has no binaries in the archive",
                    source
                );
                continue;
            };

            let architectures = source_info
                .iter()
                .filter_map(|(architecture, version)| {
                    if version != &max_version {
                        Some(*architecture)
                    } else {
                        None
                    }
                })
                .collect();
            sources_architectures.push((source, max_version, architectures));
        }

        Ok(sources_architectures)
    }
}

#[async_trait]
impl Command for NMUVersionSkew<'_> {
    async fn run(&self) -> Result<()> {
        let suite = self.options.suite.into();
        let sources = self.load_version_skew(suite)?;

        for (source, version, architectures) in sources {
            let mut source = SourceSpecifier::new(&source);
            source.with_suite(&self.options.suite);
            source.with_archive_architectures(architectures.as_ref());
            let version_without_binnmu = version.clone().without_binnmu_version();
            source.with_version(&version_without_binnmu);

            let mut binnmu = BinNMU::new(&source, "Rebuild to sync binNMU versions")?;
            binnmu.with_build_priority(self.options.build_priority);
            binnmu.with_nmu_version(version.binnmu_version().unwrap());

            let command = binnmu.build();
            println!("{}", command);
            if !self.base_options.dry_run {
                command.execute()?;
            }
        }

        Ok(())
    }

    fn downloads(&self) -> Vec<CacheEntries> {
        vec![CacheEntries::FTBFSBugs(self.options.suite.into())]
    }

    fn required_downloads(&self) -> Vec<CacheEntries> {
        vec![CacheEntries::Packages(self.options.suite.into())]
    }
}
