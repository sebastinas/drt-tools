// Copyright 2021-2023 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::{HashMap, HashSet},
    fmt,
    iter::FusedIterator,
    path::Path,
    str::FromStr,
    vec::IntoIter,
};

use anyhow::Result;
use assorted_debian_utils::{
    architectures::Architecture,
    archive::{Codename, Extension, Suite, SuiteOrCodename},
    rfc822_like,
    version::PackageVersion,
    wb::{BinNMU, SourceSpecifier, WBCommandBuilder},
};
use async_trait::async_trait;
use clap::Parser;
use indicatif::{ProgressBar, ProgressBarIter, ProgressIterator};
use itertools::Itertools;
use log::{debug, trace, warn};
use serde::Deserialize;

use crate::{
    config::{
        default_progress_style, default_progress_template, source_skip_binnmu, Cache, CacheEntries,
    },
    udd_bugs::{load_bugs_from_reader, UDDBugs},
    utils::execute_wb_commands,
    AsyncCommand, BaseOptions, Downloads,
};

#[derive(Deserialize, Debug, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
struct BinaryPackage {
    source: Option<String>,
    package: String,
    version: PackageVersion,
    architecture: Architecture,
    #[serde(rename = "Built-Using")]
    built_using: Option<String>,
    #[serde(rename = "Static-Built-Using")]
    static_built_using: Option<String>,
    #[serde(rename = "X-Cargo-Built-Using")]
    x_cargo_built_using: Option<String>,
}

#[derive(Deserialize, Debug, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
struct SourcePackage {
    package: String,
    version: PackageVersion,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(clippy::enum_variant_names)]
pub enum Field {
    BuiltUsing,
    StaticBuiltUsing,
    XCargoBuiltUsing,
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Field::BuiltUsing => write!(f, "Built-Using"),
            Field::StaticBuiltUsing => write!(f, "Static-Built-Using"),
            Field::XCargoBuiltUsing => write!(f, "X-Cargo-Built-Using"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub struct ParseError;

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid reference field")
    }
}

impl FromStr for Field {
    type Err = ParseError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "Built-Using" => Ok(Field::BuiltUsing),
            "Static-Built-Using" => Ok(Field::StaticBuiltUsing),
            "X-Cargo-Built-Using" => Ok(Field::XCargoBuiltUsing),
            _ => Err(ParseError),
        }
    }
}

#[derive(Debug, Parser)]
pub(crate) struct NMUOutdatedBuiltUsingOptions {
    /// Build priority. If specified, the binNMUs are scheduled with the given build priority. Builds with a positive priority will be built earlier.
    #[clap(long = "bp", default_value_t = -50)]
    build_priority: i32,
    /// Suite for binNMUs.
    #[clap(short, long, default_value_t = SuiteOrCodename::Suite(Suite::Unstable))]
    suite: SuiteOrCodename,
    /// Set architectures for binNMUs. If no archictures are specified, the binNMUs are scheduled with ANY.
    #[clap(short, long)]
    architecture: Option<Vec<Architecture>>,
    #[clap(long, default_value_t = Field::BuiltUsing)]
    field: Field,
}

#[derive(PartialEq, Eq, Hash)]
struct OutdatedPackage {
    source: String,
    version: PackageVersion,
    suite: Suite,
    outdated_dependency: String,
    outdated_version: PackageVersion,
}

#[derive(PartialEq, Eq)]
struct CombinedOutdatedPackage {
    source: String,
    version: PackageVersion,
    suite: Suite,
    outdated_dependencies: Vec<(String, PackageVersion)>,
}

impl PartialOrd for CombinedOutdatedPackage {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CombinedOutdatedPackage {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (&self.source, &self.version).cmp(&(&other.source, &other.version))
    }
}

fn split_dependency(dependency: &str) -> Option<(String, PackageVersion)> {
    // this should never fail unless the archive is broken
    dependency.split_once(' ').and_then(|(source, version)| {
        version
            .strip_suffix(')')
            .and_then(|version| version.strip_prefix("(= "))
            .and_then(|version| PackageVersion::try_from(version).ok())
            .map(|version| (source.to_string(), version))
    })
}

struct BinaryPackageParser<'a> {
    field: Field,
    iterator: ProgressBarIter<IntoIter<BinaryPackage>>,
    sources: &'a HashMap<String, PackageVersion>,
}

impl<'a> BinaryPackageParser<'a> {
    fn new<P>(field: Field, sources: &'a HashMap<String, PackageVersion>, path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        // read Package file
        let binary_packages: Vec<BinaryPackage> = rfc822_like::from_file(path.as_ref())?;
        let pb = ProgressBar::new(binary_packages.len() as u64);
        pb.set_style(default_progress_style().template(default_progress_template())?);
        pb.set_message(format!("Processing {}", path.as_ref().display()));
        // collect all sources with arch dependent binaries having Built-Using set and their Built-Using fields
        Ok(Self {
            field,
            iterator: binary_packages.into_iter().progress_with(pb),
            sources,
        })
    }
}

impl Iterator for BinaryPackageParser<'_> {
    type Item = (String, PackageVersion, HashSet<(String, PackageVersion)>);

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

            let source_package = if let Some(source_package) = &binary_package.source {
                match source_package.split_whitespace().next() {
                    Some(package) => package,
                    None => continue,
                }
            } else {
                // no Source set, so Source == Package
                &binary_package.package
            };

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
                            binary_package.package, self.field, dependency
                        );
                    }
                    split
                })
                .filter(|(source, version)| {
                    if let Some(max_version) = self.sources.get(source) {
                        version < max_version
                    } else {
                        // This can happen when considering packages with
                        // Static-Built-Using, but never with Built-Using. Let's
                        // rebuild those packages in any case.
                        trace!(
                            "Package '{}' refers to non-existing source package '{}'.",
                            binary_package.package,
                            source
                        );
                        true
                    }
                })
                .collect();
            // all packages in Built-Using are up to date
            if built_using.is_empty() {
                trace!(
                    "Skipping {}: all dependencies in {} are up-to-date.",
                    source_package,
                    self.field
                );
                continue;
            }

            return Some((
                source_package.into(),
                binary_package.version.without_binnmu_version(),
                built_using,
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
                .get_cache_bufreader(format!("udd-ftbfs-bugs-{codename}.yaml"))?,
        )
    }

    /// Load source packages with the highest version
    fn load_sources<P>(path: P, destination: &mut HashMap<String, PackageVersion>) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let sources: Vec<SourcePackage> = rfc822_like::from_file(path.as_ref())?;

        for source in sources {
            destination
                .entry(source.package)
                .and_modify(|version| {
                    if source.version > *version {
                        *version = source.version.clone();
                    }
                })
                .or_insert(source.version);
        }

        Ok(())
    }

    /// Load source packages from multiple suites with the highest version
    fn load_sources_for_suites(&self, suites: &[Suite]) -> Result<HashMap<String, PackageVersion>> {
        let mut ret: HashMap<String, PackageVersion> = HashMap::default();
        for suite in suites {
            Self::load_sources(self.cache.get_source_path(*suite)?, &mut ret)?;
        }
        Ok(ret)
    }

    fn load_eso(&self, field: Field, suite: Suite) -> Result<Vec<CombinedOutdatedPackage>> {
        let codename = suite.into();
        let ftbfs_bugs = self.load_bugs(codename)?;
        let source_packages = self.load_sources_for_suites(&self.expand_suite_for_sources())?;
        let mut packages = HashSet::new();
        for suite in self.expand_suite_for_binaries() {
            for path in self.cache.get_package_paths(suite, false)? {
                for (source, version, dependencies) in
                    BinaryPackageParser::new(field, &source_packages, path)?
                {
                    packages.extend(dependencies.into_iter().map(
                        |(outdated_dependency, outdated_version)| OutdatedPackage {
                            source: source.clone(),
                            version: version.clone(),
                            suite,
                            outdated_dependency,
                            outdated_version,
                        },
                    ));
                }
            }
        }

        let mut result =
            HashMap::<(String, Suite), HashSet<(PackageVersion, String, PackageVersion)>>::new();
        for outdated_package in packages {
            // skip some packages that make no sense to binNMU
            if source_skip_binnmu(&outdated_package.source) {
                debug!(
                    "Skipping {}: signed or d-i package",
                    outdated_package.source
                );
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

            result
                .entry((outdated_package.source, outdated_package.suite))
                .or_default()
                .insert((
                    outdated_package.version,
                    outdated_package.outdated_dependency,
                    outdated_package.outdated_version,
                ));
        }

        Ok(result
            .into_iter()
            .filter_map(|((source, suite), outdated_dependencies)| {
                let max_version = outdated_dependencies
                    .iter()
                    .map(|(version, _, _)| version)
                    .max()
                    .cloned()?;
                Some(CombinedOutdatedPackage {
                    source,
                    version: max_version.clone(),
                    suite,
                    outdated_dependencies: outdated_dependencies
                        .into_iter()
                        .filter_map(|(version, outdated_dependency, outdated_version)| {
                            if version == max_version {
                                Some((outdated_dependency, outdated_version))
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

    fn expand_suite_for_sources(&self) -> Vec<Suite> {
        let suite: Suite = self.options.suite.into();
        match suite {
            // when looking at testing, ignore testing-proposed-updates
            Suite::Testing(_) | Suite::Unstable | Suite::Experimental => vec![suite],
            // when looking at stable, consider stable and proposed-updates
            Suite::Stable(None) | Suite::OldStable(None) => {
                vec![suite, suite.with_extension(Extension::ProposedUpdates)]
            }
            // always consider base suite as well
            Suite::Stable(Some(_)) | Suite::OldStable(Some(_)) => {
                vec![suite.without_extension(), suite]
            }
        }
    }

    fn expand_suite_for_binaries(&self) -> Vec<Suite> {
        let suite: Suite = self.options.suite.into();
        match suite {
            // when looking at testing, ignore testing-proposed-updates
            Suite::Testing(_) | Suite::Unstable | Suite::Experimental => vec![suite],
            // when looking at stable, consider stable and proposed-updates
            Suite::Stable(None) | Suite::OldStable(None) => {
                vec![suite, suite.with_extension(Extension::ProposedUpdates)]
            }
            Suite::Stable(Some(_)) | Suite::OldStable(Some(_)) => {
                vec![suite]
            }
        }
    }
}

#[async_trait]
impl AsyncCommand for NMUOutdatedBuiltUsing<'_> {
    async fn run(&self) -> Result<()> {
        let suite = self.options.suite.into();
        let eso_sources = self.load_eso(self.options.field, suite)?;

        let mut wb_commands = Vec::new();
        for outdated_package in eso_sources {
            let mut source = SourceSpecifier::new(&outdated_package.source);
            source.with_version(&outdated_package.version);
            source.with_suite(outdated_package.suite.into());
            if let Some(architectures) = &self.options.architecture {
                source.with_archive_architectures(architectures);
            }

            let message = format!(
                "Rebuild for outdated {} ({})",
                self.options.field,
                outdated_package
                    .outdated_dependencies
                    .into_iter()
                    .map(|(source, version)| format!("{source}/{version}"))
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
        vec![CacheEntries::FTBFSBugs(self.options.suite.into())]
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
        assert_eq!(dependency.0, "rustc");
        assert_eq!(
            dependency.1,
            PackageVersion::try_from("1.70.0+dfsg1-5").unwrap()
        );
    }
}
