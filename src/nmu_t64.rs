// Copyright 2024 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{collections::HashSet, iter::FusedIterator, path::Path, vec::IntoIter};

use anyhow::{Context, Result};
use assorted_debian_utils::{
    architectures::Architecture,
    archive::{Codename, Suite},
    rfc822_like,
    version::PackageVersion,
    wb::{BinNMU, SourceSpecifier, WBCommand, WBCommandBuilder},
};
use async_trait::async_trait;
use clap::Parser;
use indicatif::{ProgressBar, ProgressBarIter, ProgressIterator};
use log::{debug, info};
use serde::Deserialize;

use crate::{
    config::{
        default_progress_style, default_progress_template, source_skip_binnmu, Cache, CacheEntries,
    },
    udd_bugs::{load_bugs_from_reader, UDDBugs},
    utils::execute_wb_commands,
    AsyncCommand, BaseOptions, Downloads,
};

#[derive(Debug, Parser)]
pub(crate) struct NMUTime64Options {
    /// Build priority. If specified, the binNMUs are scheduled with the given build priority. Builds with a positive priority will be built earlier.
    #[clap(long = "bp", default_value_t = 0)]
    build_priority: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct LibraryBinaryPackage {
    package: String,
    architecture: Architecture,
}

struct LibraryPackageParser {
    iterator: ProgressBarIter<IntoIter<LibraryBinaryPackage>>,
    next: Option<String>,
}

impl LibraryPackageParser {
    fn new<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        // read Package file
        let binary_packages: Vec<_> = rfc822_like::from_file(path.as_ref())?;
        let pb = ProgressBar::new(binary_packages.len() as u64);
        pb.set_style(default_progress_style().template(default_progress_template())?);
        pb.set_message(format!(
            "Collecting t64 libraries from {}",
            path.as_ref().display()
        ));
        Ok(Self {
            iterator: binary_packages.into_iter().progress_with(pb),
            next: None,
        })
    }
}

const T64_UNDONE: [&str; 4] = ["libcom-err2", "libss2", "libpam0g", "libuuid1"];

impl Iterator for LibraryPackageParser {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next.is_some() {
            return self.next.take();
        }

        for binary_package in self.iterator.by_ref() {
            if binary_package.architecture == Architecture::All {
                continue;
            }

            let Some(package_without_t64) = binary_package.package.strip_suffix("t64") else {
                continue;
            };
            if T64_UNDONE.contains(&package_without_t64) {
                continue;
            };
            info!("Checking packages {0} and {0}v5", package_without_t64);
            self.next = Some(format!("{}v5", package_without_t64));
            return Some(package_without_t64.into());
        }

        None
    }
}

impl FusedIterator for LibraryPackageParser {}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct BinaryPackage {
    source: Option<String>,
    package: String,
    version: PackageVersion,
    architecture: Architecture,
    depends: Option<String>,
}

struct BinaryPackageParser<'a> {
    iterator: ProgressBarIter<IntoIter<BinaryPackage>>,
    library_packages: &'a HashSet<String>,
}

impl<'a> BinaryPackageParser<'a> {
    fn new<P>(library_packages: &'a HashSet<String>, path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let binary_packages: Vec<BinaryPackage> = rfc822_like::from_file(path.as_ref())
            .with_context(|| {
                format!("Failed to parse packages from {}", path.as_ref().display())
            })?;
        let pb = ProgressBar::new(binary_packages.len() as u64);
        pb.set_style(default_progress_style().template(default_progress_template())?);
        Ok(Self {
            library_packages,
            iterator: binary_packages.into_iter().progress_with(pb),
        })
    }
}

fn extract_package_from_dependency(dependency: &str) -> &str {
    match dependency.split_once(' ') {
        Some((package, _)) => package,
        None => dependency,
    }
}

impl Iterator for BinaryPackageParser<'_> {
    type Item = (String, PackageVersion);

    fn next(&mut self) -> Option<Self::Item> {
        for binary_package in self.iterator.by_ref() {
            // skip Arch: all packages
            if binary_package.architecture == Architecture::All {
                continue;
            }
            // skip Packages without Depends
            let Some(dependencies) = binary_package.depends else {
                continue;
            };

            for dependency in dependencies
                .split(", ")
                .map(extract_package_from_dependency)
            {
                if !self.library_packages.contains(dependency) {
                    continue;
                }

                let source_package = if let Some(source_package) = &binary_package.source {
                    match source_package.split_whitespace().next() {
                        Some(package) => package.into(),
                        None => continue,
                    }
                } else {
                    // no Source set, so Source == Package
                    binary_package.package
                };

                info!(
                    "Rebuilding {} for {} on {}",
                    source_package, dependency, binary_package.architecture
                );

                return Some((source_package, binary_package.version));
            }
        }

        None
    }
}

impl FusedIterator for BinaryPackageParser<'_> {}

pub(crate) struct NMUTime64<'a> {
    cache: &'a Cache,
    base_options: &'a BaseOptions,
    options: NMUTime64Options,
}

impl<'a> NMUTime64<'a> {
    pub(crate) fn new(
        cache: &'a Cache,
        base_options: &'a BaseOptions,
        options: NMUTime64Options,
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

    fn generate_nmus(
        &self,
        architecture: Architecture,
        ftbfs_bugs: &UDDBugs,
    ) -> Result<Vec<WBCommand>> {
        let mut packages: HashSet<(String, PackageVersion)> = HashSet::new();
        let path = self.cache.get_package_path(Suite::Unstable, architecture)?;
        let library_packages: HashSet<_> = LibraryPackageParser::new(&path)?.collect();

        for (source, version) in BinaryPackageParser::new(&library_packages, path)? {
            // skip some packages that make no sense to binNMU
            if source_skip_binnmu(&source) {
                continue;
            }

            packages.insert((source, version.without_binnmu_version()));
        }

        let mut wb_commands = vec![];
        for (source, version) in packages {
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

            let mut source = SourceSpecifier::new(&source);
            source.with_version(&version);
            source.with_suite(Suite::Unstable.into());
            source.with_archive_architectures(&[architecture]);

            let mut binnmu = BinNMU::new(&source, "Rebuild for time_t")?;
            binnmu.with_build_priority(self.options.build_priority);

            wb_commands.push(binnmu.build());
        }

        Ok(wb_commands)
    }
}

#[async_trait]
impl AsyncCommand for NMUTime64<'_> {
    async fn run(&self) -> Result<()> {
        let ftbfs_bugs = self
            .load_bugs(Codename::Sid)
            .with_context(|| format!("Failed to load bugs for {}", Suite::Unstable))?;

        let mut all_wb_commands = vec![];
        for architecture in self.cache.architectures_for_suite(Suite::Unstable) {
            if architecture == Architecture::All {
                continue;
            }

            let mut wb_commands = self.generate_nmus(architecture, &ftbfs_bugs)?;
            all_wb_commands.append(&mut wb_commands);
        }

        execute_wb_commands(all_wb_commands, self.base_options).await
    }
}

impl Downloads for NMUTime64<'_> {
    fn downloads(&self) -> Vec<CacheEntries> {
        vec![CacheEntries::FTBFSBugs(Codename::Sid)]
    }

    fn required_downloads(&self) -> Vec<CacheEntries> {
        vec![CacheEntries::Packages(Suite::Unstable)]
    }
}
