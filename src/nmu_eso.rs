// Copyright 2021-2025 Sebastian Ramacher
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
    archive::{Extension, Suite, SuiteOrCodename, WithExtension},
    package::{PackageName, VersionedPackage},
    rfc822_like,
    version::PackageVersion,
    wb::{BinNMU, SourceSpecifier, WBArchitecture, WBCommandBuilder},
};
use async_trait::async_trait;
use indicatif::{ProgressBar, ProgressBarIter, ProgressIterator};
use itertools::Itertools;
use log::{debug, trace, warn};
use serde::Deserialize;

use crate::{
    AsyncCommand, Downloads,
    cli::{BaseOptions, Field, NMUOutdatedBuiltUsingOptions},
    config::{
        Cache, CacheEntries, default_progress_style, default_progress_template, source_skip_binnmu,
    },
    source_packages::{self, SourcePackages},
    udd_bugs::UDDBugs,
    utils::execute_wb_commands,
};

#[derive(Deserialize, Debug, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
struct BinaryPackage {
    #[serde(flatten)]
    package: source_packages::BinaryPackage,
    architecture: Architecture,
    #[serde(rename = "Built-Using")]
    built_using: Option<String>,
    #[serde(rename = "Static-Built-Using")]
    static_built_using: Option<String>,
    #[serde(rename = "X-Cargo-Built-Using")]
    x_cargo_built_using: Option<String>,
}

#[derive(PartialEq, Eq, Hash)]
struct OutdatedPackage {
    source: VersionedPackage,
    suite: Suite,
    outdated_dependency: VersionedPackage,
    architecture: WBArchitecture,
}

#[derive(PartialEq, Eq)]
struct CombinedOutdatedPackage {
    source: VersionedPackage,
    suite: Suite,
    architecture: WBArchitecture,
    outdated_dependencies: Vec<VersionedPackage>,
}

impl PartialOrd for CombinedOutdatedPackage {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CombinedOutdatedPackage {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.source.cmp(&other.source)
    }
}

fn split_dependency(dependency: &str) -> Option<VersionedPackage> {
    // this should never fail unless the archive is broken
    dependency.split_once(' ').and_then(|(source, version)| {
        let version = version
            .strip_suffix(')')
            .and_then(|version| version.strip_prefix("(= "))
            .and_then(|version| PackageVersion::try_from(version).ok());
        let source = PackageName::try_from(source);
        match (source, version) {
            (Ok(source), Some(version)) => Some(VersionedPackage {
                package: source,
                version,
            }),
            _ => None,
        }
    })
}

struct BinaryPackageParser<'a> {
    field: Field,
    iterator: ProgressBarIter<IntoIter<BinaryPackage>>,
    sources: &'a SourcePackages,
}

impl<'a> BinaryPackageParser<'a> {
    fn new<P>(field: Field, sources: &'a SourcePackages, path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        // read Package file
        let binary_packages: Vec<BinaryPackage> = rfc822_like::from_file(path.as_ref())?;
        let pb = ProgressBar::new(binary_packages.len() as u64);
        pb.set_style(default_progress_style().template(default_progress_template())?);
        pb.set_message(format!("Processing {}", path.as_ref().display()));
        // collect all sources with arch dependent binaries having Built-Using set and their Built-Using fields refer to ESO sources
        Ok(Self {
            field,
            iterator: binary_packages.into_iter().progress_with(pb),
            sources,
        })
    }
}

struct OutdatedSourcePackage {
    source: VersionedPackage,
    built_using: HashSet<VersionedPackage>,
    architecture: WBArchitecture,
}

impl Iterator for BinaryPackageParser<'_> {
    type Item = OutdatedSourcePackage;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(binary_package) = self.iterator.next() {
            // skip Arch: all packages
            if binary_package.architecture == Architecture::All {
                continue;
            }
            // skip packages without Built-Using
            let Some(ref built_using) = (match self.field {
                Field::BuiltUsing => binary_package.built_using,
                Field::StaticBuiltUsing => binary_package.static_built_using,
                Field::XCargoBuiltUsing => binary_package.x_cargo_built_using,
            }) else {
                continue;
            };

            let (source_package, version) = binary_package.package.name_and_version();

            // remove trailing spaces found in X-Cargo-Built-Using
            let built_using = built_using.strip_suffix(' ').unwrap_or(built_using);
            // remove trailing commas found in X-Cargo-Built-Using
            let built_using = built_using.strip_suffix(',').unwrap_or(built_using);

            let built_using: HashSet<_> = built_using
                .split(", ")
                .filter_map(|dependency| {
                    let split = split_dependency(dependency);
                    if split.is_none() {
                        warn!(
                            "Package '{}' contains invalid dependency in {}: {}",
                            binary_package.package.package, self.field, dependency
                        );
                    }
                    split
                })
                .filter(|source| {
                    if let Some(max_version) = self.sources.version(&source.package) {
                        source.version < *max_version
                    } else {
                        // This can happen when considering packages with
                        // Static-Built-Using, but never with Built-Using. Let's
                        // rebuild those packages in any case.
                        trace!(
                            "Package '{}' refers to non-existing source package '{}'.",
                            binary_package.package.package, source.package
                        );
                        true
                    }
                })
                .collect();
            // all packages in Built-Using are up to date
            if built_using.is_empty() {
                trace!(
                    "Skipping {}: all dependencies in {} are up-to-date.",
                    source_package, self.field
                );
                continue;
            }

            // if the package builds any MA: same packages, schedule binNMUs with ANY
            let architecture = if self.sources.is_ma_same(&source_package) {
                WBArchitecture::Any
            } else {
                WBArchitecture::Architecture(binary_package.architecture)
            };
            return Some(OutdatedSourcePackage {
                source: VersionedPackage {
                    package: source_package.clone(),
                    version: version.without_binnmu_version(),
                },
                built_using,
                architecture,
            });
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

    /// Load source packages from multiple suites with the highest version
    fn load_sources_for_suites(&self, suites: &[SuiteOrCodename]) -> Result<SourcePackages> {
        let paths: Result<Vec<_>> = suites
            .iter()
            .map(|suite| self.cache.get_package_paths(*suite, false))
            .flatten_ok()
            .collect();
        let sources: Result<Vec<_>> = suites
            .iter()
            .map(|suite| self.cache.get_source_path(*suite))
            .collect();
        SourcePackages::new_with_source(&sources?, &paths?)
    }

    fn load_eso(
        &self,
        field: Field,
        suite: SuiteOrCodename,
    ) -> Result<Vec<CombinedOutdatedPackage>> {
        let ftbfs_bugs = UDDBugs::load_for_codename(self.cache, suite)?;
        let source_packages = self.load_sources_for_suites(&self.expand_suite_for_sources())?;

        // collect outdated binary packages
        let mut packages = HashSet::new();
        for suite in self.expand_suite_for_binaries() {
            let converted_suite = suite.into();
            for path in self.cache.get_package_paths(suite, false)? {
                for OutdatedSourcePackage {
                    source,
                    built_using: dependencies,
                    architecture,
                } in BinaryPackageParser::new(field, &source_packages, path)?
                {
                    // skip some packages that make no sense to binNMU
                    if source_skip_binnmu(source.package.as_ref()) {
                        debug!(
                            "Skipping {}: signed or d-i package or otherwise not binNMU-able",
                            source.package
                        );
                        continue;
                    }

                    packages.extend(dependencies.into_iter().map(|outdated_dependency| {
                        OutdatedPackage {
                            source: source.clone(),
                            suite: converted_suite,
                            outdated_dependency,
                            architecture,
                        }
                    }));
                }
            }
        }

        let mut result = HashMap::<
            (PackageName, Suite, WBArchitecture),
            HashSet<(PackageVersion, VersionedPackage)>,
        >::new();
        for outdated_package in packages {
            // check if package FTBFS
            if let Some(bugs) = ftbfs_bugs.bugs_for_source(&outdated_package.source.package) {
                println!(
                    "# Skipping {} due to FTBFS bugs ...",
                    outdated_package.source.package
                );
                for bug in bugs {
                    debug!(
                        "Skipping {}: #{} - {}: {}",
                        outdated_package.source.package, bug.id, bug.severity, bug.title
                    );
                }
                continue;
            }

            result
                .entry((
                    outdated_package.source.package,
                    outdated_package.suite,
                    outdated_package.architecture,
                ))
                .or_default()
                .insert((
                    outdated_package.source.version,
                    outdated_package.outdated_dependency,
                ));
        }

        Ok(result
            .into_iter()
            .filter_map(|((source, suite, architecture), outdated_dependencies)| {
                let max_version = outdated_dependencies
                    .iter()
                    .map(|(version, _)| version)
                    .max()
                    .cloned()?;
                Some(CombinedOutdatedPackage {
                    source: VersionedPackage {
                        package: source,
                        version: max_version.clone(),
                    },
                    suite,
                    architecture,
                    outdated_dependencies: outdated_dependencies
                        .into_iter()
                        .filter_map(|(version, outdated_dependency)| {
                            if version == max_version {
                                Some(outdated_dependency)
                            } else {
                                None
                            }
                        })
                        .sorted()
                        .collect(),
                })
            })
            .sorted()
            .collect())
    }

    fn expand_suite_for_sources(&self) -> Vec<SuiteOrCodename> {
        let suite: Suite = self.options.suite.into();
        match suite {
            // when looking at testing, ignore testing-proposed-updates
            Suite::Testing(_) | Suite::Unstable | Suite::Experimental => vec![self.options.suite],
            // when looking at stable, consider stable and proposed-updates
            Suite::Stable(None) | Suite::OldStable(None) => {
                vec![
                    self.options.suite,
                    self.options
                        .suite
                        .with_extension(Extension::ProposedUpdates),
                ]
            }
            // always consider base suite as well
            Suite::Stable(Some(_)) | Suite::OldStable(Some(_)) => {
                vec![self.options.suite.without_extension(), self.options.suite]
            }
        }
    }

    fn expand_suite_for_binaries(&self) -> Vec<SuiteOrCodename> {
        let suite: Suite = self.options.suite.into();
        match suite {
            // when looking at testing, ignore testing-proposed-updates
            Suite::Testing(_) | Suite::Unstable | Suite::Experimental => vec![self.options.suite],
            // when looking at stable, consider stable and proposed-updates
            Suite::Stable(None) | Suite::OldStable(None) => {
                vec![
                    self.options.suite,
                    self.options
                        .suite
                        .with_extension(Extension::ProposedUpdates),
                ]
            }
            Suite::Stable(Some(_)) | Suite::OldStable(Some(_)) => {
                vec![self.options.suite]
            }
        }
    }
}

#[async_trait]
impl AsyncCommand for NMUOutdatedBuiltUsing<'_> {
    async fn run(&self) -> Result<()> {
        let eso_sources = self.load_eso(self.options.field, self.options.suite)?;

        let mut wb_commands = Vec::new();
        for outdated_package in eso_sources {
            let mut source = SourceSpecifier::new(&outdated_package.source.package);
            source.with_version(&outdated_package.source.version);
            source.with_suite(outdated_package.suite.into());
            source.with_architectures(&[outdated_package.architecture]);

            let message = format!(
                "Rebuild for outdated {} ({})",
                self.options.field,
                outdated_package
                    .outdated_dependencies
                    .into_iter()
                    .map(|source| format!("{}/{}", source.package, source.version))
                    .join(", ")
            );
            let mut binnmu = BinNMU::new(&source, &message)?;
            binnmu.with_build_priority(self.options.build_priority);

            wb_commands.push(binnmu.build());
        }

        execute_wb_commands(wb_commands, self.base_options).await
    }
}

impl Downloads for NMUOutdatedBuiltUsing<'_> {
    fn downloads(&self) -> Vec<CacheEntries> {
        vec![CacheEntries::FTBFSBugs(self.options.suite)]
    }

    fn required_downloads(&self) -> Vec<CacheEntries> {
        self.expand_suite_for_binaries()
            .into_iter()
            .map(CacheEntries::Packages)
            .chain(
                self.expand_suite_for_sources()
                    .into_iter()
                    .map(CacheEntries::Sources),
            )
            .collect()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn dependencies() {
        assert!(split_dependency("( =)").is_none());

        let dependency = split_dependency("rustc (= 1.70.0+dfsg1-5)").unwrap();
        assert_eq!(dependency.package, "rustc");
        assert_eq!(dependency.version, "1.70.0+dfsg1-5");
    }
}
