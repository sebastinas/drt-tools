// Copyright 2023 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::{HashMap, HashSet},
    iter::FusedIterator,
    path::Path,
    vec::IntoIter,
};

use anyhow::{Context, Result};
use assorted_debian_utils::{
    architectures::Architecture,
    archive::{Codename, MultiArch, SuiteOrCodename},
    package::PackageName,
    rfc822_like,
    version::PackageVersion,
    wb::{BinNMU, SourceSpecifier, WBCommandBuilder},
};
use async_trait::async_trait;
use indicatif::{ProgressBar, ProgressBarIter, ProgressIterator};
use itertools::Itertools;
use log::{debug, error};
use serde::Deserialize;

use crate::{
    AsyncCommand, Downloads,
    cli::{BaseOptions, NMUVersionSkewOptions},
    config::{
        Cache, CacheEntries, default_progress_style, default_progress_template, source_skip_binnmu,
    },
    source_packages,
    udd_bugs::{UDDBugs, load_bugs_from_reader},
    utils::execute_wb_commands,
};

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct BinaryPackage {
    #[serde(flatten)]
    package: source_packages::BinaryPackage,
    architecture: Architecture,
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
        let binary_packages: Vec<BinaryPackage> = rfc822_like::from_file(path.as_ref())
            .with_context(|| {
                format!("Failed to parse packages from {}", path.as_ref().display())
            })?;
        let pb = ProgressBar::new(binary_packages.len() as u64);
        pb.set_style(default_progress_style().template(default_progress_template())?);
        pb.set_message(format!("Processing {}", path.as_ref().display()));
        // collect all sources with arch dependent binaries having Built-Using set and their Built-Using fields
        Ok(Self {
            iterator: binary_packages.into_iter().progress_with(pb),
        })
    }
}

impl Iterator for BinaryPackageParser {
    type Item = (PackageName, Architecture, PackageVersion);

    fn next(&mut self) -> Option<Self::Item> {
        for binary_package in self.iterator.by_ref() {
            // skip packages that are not MA: same
            if binary_package.package.multi_arch != Some(MultiArch::Same) {
                continue;
            }
            // skip Arch: all packages
            if binary_package.architecture == Architecture::All {
                continue;
            }

            let (source_package, _) = binary_package.package.name_and_version();
            return Some((
                source_package.clone(),
                binary_package.architecture,
                binary_package.package.version,
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
                .get_cache_bufreader(format!("udd-ftbfs-bugs-{codename}.yaml"))?,
        )
    }

    fn load_version_skew(
        &self,
        suite: SuiteOrCodename,
    ) -> Result<Vec<(PackageName, PackageVersion, Vec<Architecture>)>> {
        let ftbfs_bugs = self
            .load_bugs(suite.into())
            .with_context(|| format!("Failed to load bugs for {suite}"))?;
        let mut packages: HashMap<PackageName, HashSet<(Architecture, PackageVersion)>> =
            HashMap::new();
        for path in self.cache.get_package_paths(suite, false)? {
            for (source, architecture, version) in BinaryPackageParser::new(path)? {
                // skip some packages that make no sense to binNMU
                if source_skip_binnmu(source.as_ref()) {
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
            Vec::<(PackageName, PackageVersion, Vec<Architecture>)>::default();
        for (source, source_info) in packages.into_iter().sorted_by_key(|value| value.0.clone()) {
            let mut max_version_per_architecture: HashMap<Architecture, PackageVersion> =
                HashMap::default();
            for (architecture, version) in &source_info {
                if let Some(max_version) = max_version_per_architecture.get_mut(architecture) {
                    if version > max_version {
                        *max_version = version.clone();
                    }
                } else {
                    max_version_per_architecture.insert(*architecture, version.clone());
                }
            }

            let all_versions: HashSet<_> = max_version_per_architecture.values().cloned().collect();
            if all_versions.len() == 1 {
                debug!("Skipping {}: package is in sync", source);
                continue;
            }

            let all_versions_without_binnmu: HashSet<_> = max_version_per_architecture
                .values()
                .map(|v| (*v).clone().without_binnmu_version())
                .collect();
            if all_versions_without_binnmu.len() != 1 {
                debug!(
                    "Skipping {}: package is out-of-date on some architecture",
                    source
                );
                continue;
            }

            // check if package FTBFS
            if let Some(bugs) = ftbfs_bugs.bugs_for_source(source.as_ref()) {
                println!("# Skipping {source} due to FTBFS bugs ...");
                for bug in bugs {
                    debug!(
                        "Skipping {}: #{} - {}: {}",
                        source, bug.id, bug.severity, bug.title
                    );
                }
                continue;
            }

            let Some(max_version) = max_version_per_architecture
                .values()
                .max()
                .map(|v| (*v).clone())
            else {
                error!(
                    "Skipping {}: package has no binaries in the archive",
                    source
                );
                continue;
            };
            debug!("Max {} version: {}", source, max_version);

            let architectures = max_version_per_architecture
                .iter()
                .filter_map(|(architecture, version)| {
                    if version == &max_version {
                        None
                    } else {
                        Some(*architecture)
                    }
                })
                .collect();
            sources_architectures.push((source, max_version, architectures));
        }

        Ok(sources_architectures)
    }
}

#[async_trait]
impl AsyncCommand for NMUVersionSkew<'_> {
    async fn run(&self) -> Result<()> {
        let sources = self.load_version_skew(self.options.suite)?;

        let mut wb_commands = Vec::new();
        for (source, version, architectures) in sources {
            let mut source = SourceSpecifier::new(&source);
            source.with_suite(self.options.suite);
            source.with_archive_architectures(architectures.as_ref());
            let version_without_binnmu = version.clone().without_binnmu_version();
            source.with_version(&version_without_binnmu);

            let mut binnmu = BinNMU::new(&source, "Rebuild to sync binNMU versions")?;
            binnmu.with_build_priority(self.options.build_priority);
            let Some(binnmu_version) = version.binnmu_version() else {
                error!("Skipping {}: package version has no binNMU.", source);
                continue;
            };
            binnmu.with_nmu_version(binnmu_version);

            wb_commands.push(binnmu.build());
        }

        execute_wb_commands(wb_commands, self.base_options).await
    }
}

impl Downloads for NMUVersionSkew<'_> {
    fn downloads(&self) -> Vec<CacheEntries> {
        vec![CacheEntries::FTBFSBugs(self.options.suite)]
    }

    fn required_downloads(&self) -> Vec<CacheEntries> {
        vec![CacheEntries::Packages(self.options.suite)]
    }
}
