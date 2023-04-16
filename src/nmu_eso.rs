// Copyright 2021-2023 Sebastian Ramacher
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
    archive::{Codename, Suite, SuiteOrCodename},
    rfc822_like,
    version::PackageVersion,
    wb::{BinNMU, SourceSpecifier, WBCommandBuilder},
};
use async_trait::async_trait;
use clap::Parser;
use indicatif::{ProgressBar, ProgressBarIter, ProgressIterator};
use itertools::{sorted, Itertools};
use log::{debug, trace};
use serde::Deserialize;

use crate::{
    config::{default_progress_style, Cache, CacheEntries},
    udd_bugs::{load_bugs_from_reader, UDDBugs},
    BaseOptions, Command,
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

#[derive(Deserialize, Debug, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
enum ExtraSourceOnly {
    Yes,
}

#[derive(Deserialize, Debug, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
struct SourcePackage {
    package: String,
    version: PackageVersion,
    #[serde(rename = "Extra-Source-Only")]
    extra_source_only: Option<ExtraSourceOnly>,
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

#[derive(PartialEq, Eq, Hash)]
struct OutdatedPackage {
    source: String,
    outdated_dependency: String,
    outdated_version: PackageVersion,
}

struct CombinedOutdatedPackage {
    source: String,
    outdated_dependencies: Vec<(String, PackageVersion)>,
}

impl PartialEq for CombinedOutdatedPackage {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source && self.outdated_dependencies == other.outdated_dependencies
    }
}

impl Eq for CombinedOutdatedPackage {}

impl PartialOrd for CombinedOutdatedPackage {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.source.partial_cmp(&other.source)
    }
}

impl Ord for CombinedOutdatedPackage {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.source.cmp(&other.source)
    }
}

fn split_dependency(dependency: &str) -> Option<(String, PackageVersion)> {
    // this should never fail unless the archive is broken
    dependency.split_once(' ').and_then(|(source, version)| {
        PackageVersion::try_from(&version[3..version.len() - 1])
            .map(|version| (source.to_string(), version))
            .ok()
    })
}

struct BinaryPackageParser<'a> {
    iterator: ProgressBarIter<IntoIter<BinaryPackage>>,
    eso_sources: &'a HashSet<(String, PackageVersion)>,
}

impl<'a> BinaryPackageParser<'a> {
    fn new<P>(eso_sources: &'a HashSet<(String, PackageVersion)>, path: P) -> Result<Self>
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
            eso_sources,
        })
    }
}

impl Iterator for BinaryPackageParser<'_> {
    type Item = (String, HashSet<(String, PackageVersion)>);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(binary_package) = self.iterator.next() {
            // skip Arch: all packages
            if binary_package.architecture == Architecture::All {
                continue;
            }
            // skip packages without Built-Using
            let Some(ref built_using) = binary_package.built_using else {
                continue;
            };

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
                built_using
                    .split(", ")
                    .filter_map(split_dependency)
                    .filter(|dependency| self.eso_sources.contains(dependency))
                    .collect(),
            ));
        }

        None
    }
}

impl FusedIterator for BinaryPackageParser<'_> {}

pub(crate) struct NMUOutdatedBuiltUsing<'a> {
    cache: &'a Cache,
    base_options: &'a BaseOptions,
    options: NMUOutdatedBuiltUsingOptions,
}

impl<'a> NMUOutdatedBuiltUsing<'a> {
    pub(crate) fn new(
        cache: &'a Cache,
        base_options: &'a BaseOptions,
        options: NMUOutdatedBuiltUsingOptions,
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

    fn load_extra_sources<P>(path: P) -> Result<HashSet<(String, PackageVersion)>>
    where
        P: AsRef<Path>,
    {
        let sources: Vec<SourcePackage> = rfc822_like::from_file(path.as_ref())?;
        Ok(sources
            .into_iter()
            .filter(|source| source.extra_source_only.is_some())
            .map(|source| {
                trace!(
                    "Found outdated source package: {}/{}",
                    source.package,
                    source.version
                );
                (source.package, source.version)
            })
            .collect())
    }

    fn load_eso(&self, suite: Suite) -> Result<Vec<CombinedOutdatedPackage>> {
        let codename = suite.into();
        let ftbfs_bugs = self.load_bugs(codename)?;
        let eso_sources = Self::load_extra_sources(self.cache.get_source_path(suite)?)?;
        let mut packages = HashSet::new();
        for path in self.cache.get_package_paths(suite, false)? {
            for (source, dependencies) in BinaryPackageParser::new(&eso_sources, path)? {
                packages.extend(dependencies.into_iter().map(
                    |(outdated_dependency, outdated_version)| OutdatedPackage {
                        source: source.clone(),
                        outdated_dependency,
                        outdated_version,
                    },
                ))
            }
        }

        let mut result = HashMap::<String, HashSet<(String, PackageVersion)>>::new();
        for outdated_package in packages {
            // skip some packages that make no sense to binNMU
            if outdated_package.source == "debian-installer"
                || outdated_package.source == "debian-installer-netboot-images"
            {
                debug!(
                    "Skipping {}: debian-installer or debian-installer-netboot-images",
                    outdated_package.source
                );
                continue;
            }
            // skip grub/linux/... signed packages
            if outdated_package.source.ends_with("-signed")
                && (outdated_package.source.starts_with("grub-")
                    || outdated_package.source.starts_with("linux-")
                    || outdated_package.source.starts_with("shim-")
                    || outdated_package.source.starts_with("fwupd-"))
            {
                debug!("Skipping {}: signed package", outdated_package.source);
                continue;
            }

            // check if package FTBFS
            if let Some(bugs) = ftbfs_bugs.bugs_for_source(&outdated_package.source) {
                println!(
                    "# Skipping {} due to FTBFS bugs ...",
                    outdated_package.source
                );
                for bug in bugs {
                    debug!(
                        "Skipping {}: #{} - {}: {}",
                        outdated_package.source, bug.id, bug.severity, bug.title
                    );
                }
                continue;
            }

            result.entry(outdated_package.source).or_default().insert((
                outdated_package.outdated_dependency,
                outdated_package.outdated_version,
            ));
        }

        Ok(
            sorted(result.into_iter().map(|(source, outdated_dependencies)| {
                CombinedOutdatedPackage {
                    source,
                    outdated_dependencies: outdated_dependencies.into_iter().sorted().collect(),
                }
            }))
            .collect(),
        )
    }
}

#[async_trait]
impl Command for NMUOutdatedBuiltUsing<'_> {
    async fn run(&self) -> Result<()> {
        let suite = self.options.suite.into();
        let eso_sources = self.load_eso(suite)?;

        for outdated_package in eso_sources {
            let mut source = SourceSpecifier::new(&outdated_package.source);
            source.with_suite(&self.options.suite);
            if let Some(architectures) = &self.options.architecture {
                source.with_archive_architectures(architectures);
            }

            let message = format!(
                "Rebuild for outdated Built-Using ({})",
                outdated_package
                    .outdated_dependencies
                    .into_iter()
                    .map(|(source, version)| format!("{}/{}", source, version))
                    .join(", ")
            );
            let mut binnmu = BinNMU::new(&source, &message)?;
            binnmu.with_build_priority(self.options.build_priority);

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
        vec![
            CacheEntries::Packages(self.options.suite.into()),
            CacheEntries::Sources(self.options.suite.into()),
        ]
    }
}
