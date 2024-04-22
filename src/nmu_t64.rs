// Copyright 2024 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::{HashMap, HashSet},
    iter::FusedIterator,
    path::Path,
    vec::IntoIter,
};

use anyhow::{anyhow, Context, Result};
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
    /// Skip packages not in testing
    #[clap(long = "skip-not-in-testing")]
    skip_not_in_testing: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct LibraryBinaryPackage {
    package: String,
    architecture: Architecture,
}

struct LibraryPackageParser {
    iterator: ProgressBarIter<IntoIter<LibraryBinaryPackage>>,
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
        })
    }
}

const T64_UNDONE: [&str; 18] = [
    "libcom-err2t64",
    "libjellyfish-2.0-2t64",
    "libosmo-gsup-client0t64",
    "libosmo-hnbap0t64",
    "libosmo-mslookup0t64",
    "libosmo-ranap5t64",
    "libosmo-rua0t64",
    "libosmo-sabp1t64",
    "libosmo-sigtran7t64",
    "libosmomtp0t64",
    "libosmonetif8t64",
    "libosmosccp0t64",
    "libosmoxua0t64",
    "libpam0t64",
    "libshisa0t64",
    "libshish0t64",
    "libss2t64",
    "libuuid0t64",
];
const T64_SUFFIXES: [&str; 9] = [
    "", "-gnutls", "-heimdal", "-mesa", "-search", "-qt", "-gcrypt", "-nss", "-openssl",
];
const LIB_SUFFIXES: [&str; 14] = [
    "", "c202", "c2", "c2a", "a", "b", "c", "d", "e", "g", "ldbl", "v5", "gf", "debian",
];

impl Iterator for LibraryPackageParser {
    type Item = Vec<String>;

    fn next(&mut self) -> Option<Self::Item> {
        for binary_package in self.iterator.by_ref() {
            if binary_package.architecture == Architecture::All {
                continue;
            }
            // t64 changes were reverted, so check if packages depend on the t64 library package instead
            if T64_UNDONE
                .binary_search(&binary_package.package.as_ref())
                .is_ok()
            {
                info!("Checking {}", binary_package.package);
                return Some(vec![binary_package.package.clone()]);
            }

            // special packages
            if binary_package.package == "libc-ares2" {
                info!("Checking {}", binary_package.package);
                return Some(vec![binary_package.package.clone()]);
            }

            for t64_suffix in T64_SUFFIXES {
                let Some(package_without_t64) = binary_package
                    .package
                    .strip_suffix(&format!("t64{}", t64_suffix))
                else {
                    continue;
                };
                return Some(
                    LIB_SUFFIXES
                        .iter()
                        .map(|suffix| {
                            info!("Checking {}{}{}", package_without_t64, suffix, t64_suffix);
                            format!("{}{}{}", package_without_t64, suffix, t64_suffix)
                        })
                        .collect(),
                );
            }
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
    #[serde(default)]
    depends: String,
    #[serde(rename = "Pre-Depends", default)]
    pre_depends: String,
}

impl BinaryPackage {
    fn source_and_version(&self) -> Result<(&str, PackageVersion)> {
        if let Some(ref source) = self.source {
            match source.split_once(' ') {
                Some((source, version)) => version
                    .strip_prefix('(')
                    .and_then(|v| v.strip_suffix(')'))
                    .ok_or(anyhow!("invalid binary package"))
                    .and_then(|v| PackageVersion::try_from(v).context("invalid binary package"))
                    .map(|v| (source, v)),
                None => Ok((source, self.version.clone())),
            }
        } else {
            Ok((&self.package, self.version.clone()))
        }
    }
}

struct BinaryPackageParser<'a> {
    iterator: ProgressBarIter<IntoIter<BinaryPackage>>,
    library_packages: &'a HashSet<String>,
    source_packages: &'a HashMap<String, PackageVersion>,
    skip_arch_all: bool,
}

impl<'a> BinaryPackageParser<'a> {
    fn new<P>(
        path: P,
        library_packages: &'a HashSet<String>,
        source_packages: &'a HashMap<String, PackageVersion>,
        skip_arch_all: bool,
    ) -> Result<Self>
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
            source_packages,
            iterator: binary_packages.into_iter().progress_with(pb),
            skip_arch_all,
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
            if self.skip_arch_all && binary_package.architecture == Architecture::All {
                continue;
            }

            let Ok((source, version)) = binary_package.source_and_version() else {
                continue;
            };
            if let Some(current_version) = self.source_packages.get(source) {
                if version < *current_version {
                    debug!("Skipping src:{}/{}: out-of-date", source, version);
                    continue;
                }
            } else {
                debug!("Skipping src:{}/{}: removed", source, version);
                continue;
            }

            for dependency in binary_package
                .depends
                .split(", ")
                .map(extract_package_from_dependency)
                .chain(
                    binary_package
                        .pre_depends
                        .split(", ")
                        .map(extract_package_from_dependency),
                )
            {
                if !self.library_packages.contains(dependency) {
                    continue;
                }

                info!(
                    "Rebuilding src:{}/{} ({}) for {} on {}",
                    source,
                    version,
                    binary_package.package,
                    dependency,
                    binary_package.architecture
                );

                return Some((source.to_string(), version));
            }
        }

        None
    }
}

impl FusedIterator for BinaryPackageParser<'_> {}

#[derive(Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
enum ExtraSourceOnly {
    #[default]
    No,
    Yes,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct SourcePackage {
    package: String,
    version: PackageVersion,
    #[serde(default)]
    extra_source_only: ExtraSourceOnly,
}

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

    fn load_sources(&self, suite: Suite) -> Result<HashMap<String, PackageVersion>> {
        let sources: Vec<SourcePackage> = rfc822_like::from_reader(
            self.cache
                .get_cache_bufreader(self.cache.get_source_path(suite)?)?,
        )?;

        let mut source_versions = HashMap::default();
        for source in sources {
            if source.extra_source_only == ExtraSourceOnly::Yes {
                continue;
            }

            if let Some(old_version) = source_versions.get_mut(&source.package) {
                if *old_version < source.version {
                    *old_version = source.version;
                }
            } else {
                source_versions.insert(source.package, source.version);
            }
        }

        Ok(source_versions)
    }

    fn generate_nmus(
        &self,
        architecture: Architecture,
        ftbfs_bugs: &UDDBugs,
        source_packages: &HashMap<String, PackageVersion>,
        testing_source_packages: &HashMap<String, PackageVersion>,
    ) -> Result<Vec<WBCommand>> {
        let mut packages: HashSet<(String, PackageVersion)> = HashSet::new();
        let path = self.cache.get_package_path(Suite::Unstable, architecture)?;
        let library_packages: HashSet<_> = LibraryPackageParser::new(&path)?.flatten().collect();

        for (source, version) in
            BinaryPackageParser::new(path, &library_packages, source_packages, true)?
        {
            // skip some packages that make no sense to binNMU
            if source_skip_binnmu(&source) {
                info!("Skipping {}: not binNMU-able", source);
                continue;
            }

            // skip packages not in testing
            if self.options.skip_not_in_testing && !testing_source_packages.contains_key(&source) {
                info!("Skipping {}: not in testing", source);
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

    fn list_arch_all_packages(
        &self,
        source_packages: &HashMap<String, PackageVersion>,
        testing_source_packages: &HashMap<String, PackageVersion>,
    ) -> Result<()> {
        let mut library_package_parsers = vec![];
        for architecture in self.cache.architectures_for_suite(Suite::Unstable) {
            if architecture == Architecture::All {
                continue;
            }

            let path = self.cache.get_package_path(Suite::Unstable, architecture)?;
            library_package_parsers.push(LibraryPackageParser::new(&path)?.flatten());
        }

        let library_packages: HashSet<_> = library_package_parsers.into_iter().flatten().collect();
        for (source, version) in BinaryPackageParser::new(
            self.cache
                .get_package_path(Suite::Unstable, Architecture::All)?,
            &library_packages,
            source_packages,
            false,
        )? {
            // skip packages not in testing
            if self.options.skip_not_in_testing && !testing_source_packages.contains_key(&source) {
                info!("Skipping {}: not in testing", source);
                continue;
            }

            println!("# reportbug --src {0} --package-version={1} --no-cc-menu --no-tags-menu --subject=\"{0}: arch:all package depends on pre-t64 library\"", source, version);
        }

        Ok(())
    }
}

#[async_trait]
impl AsyncCommand for NMUTime64<'_> {
    async fn run(&self) -> Result<()> {
        let ftbfs_bugs = self
            .load_bugs(Codename::Sid)
            .with_context(|| format!("Failed to load bugs for {}", Suite::Unstable))?;

        let source_packages = self.load_sources(Suite::Unstable)?;
        let testing_source_packages = self.load_sources(Suite::Testing(None))?;

        let mut all_wb_commands = vec![];
        for architecture in self.cache.architectures_for_suite(Suite::Unstable) {
            if architecture == Architecture::All {
                self.list_arch_all_packages(&source_packages, &testing_source_packages)?;
            } else {
                let mut wb_commands = self.generate_nmus(
                    architecture,
                    &ftbfs_bugs,
                    &source_packages,
                    &testing_source_packages,
                )?;
                all_wb_commands.append(&mut wb_commands);
            }
        }

        execute_wb_commands(all_wb_commands, self.base_options).await
    }
}

impl Downloads for NMUTime64<'_> {
    fn downloads(&self) -> Vec<CacheEntries> {
        vec![CacheEntries::FTBFSBugs(Codename::Sid)]
    }

    fn required_downloads(&self) -> Vec<CacheEntries> {
        vec![
            CacheEntries::Packages(Suite::Unstable),
            CacheEntries::Sources(Suite::Unstable),
            CacheEntries::Sources(Suite::Testing(None)),
        ]
    }
}
