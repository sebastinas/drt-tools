// Copyright 2021-2025 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    iter::FusedIterator,
    path::Path,
    vec::IntoIter,
};

use anyhow::Result;
use assorted_debian_utils::{
    architectures::Architecture,
    archive::{Extension, Suite, SuiteOrCodename, WithExtension},
    package::{
        PackageName, PackageRelationship, Relationship, VersionRelationship, VersionedPackage,
    },
    rfc822_like,
    version::PackageVersion,
    wb::{BinNMU, SourceSpecifier, WBArchitecture, WBCommand, WBCommandBuilder},
};
use async_trait::async_trait;
use indicatif::{ProgressBar, ProgressBarIter, ProgressIterator};
use itertools::Itertools;
use log::{debug, trace, warn};
use serde::{Deserialize, Deserializer, de};

use crate::{
    AsyncCommand, Downloads,
    cli::{BaseOptions, Field, NMUOutdatedBuiltUsingOptions},
    config::{
        Cache, CacheEntries, CachePaths, default_progress_style, default_progress_template,
        source_skip_binnmu,
    },
    source_packages::{self, SourcePackages},
    udd_bugs::UDDBugs,
    utils::execute_wb_commands,
};

// this is a workaround for bookworm; after the release of bookworm it can be dropped
fn deserialize_package_relationships<'de, D>(
    deserializer: D,
) -> Result<Vec<PackageRelationship>, D::Error>
where
    D: Deserializer<'de>,
{
    struct Visitor {}

    impl de::Visitor<'_> for Visitor {
        type Value = Vec<PackageRelationship>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(formatter, "a package relantionship")
        }

        fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            // remove trailing spaces found in X-Cargo-Built-Using
            let s = s.strip_suffix(' ').unwrap_or(s);
            // remove trailing commas found in X-Cargo-Built-Using
            let s = s.strip_suffix(',').unwrap_or(s);
            s.split(',')
                .map(|r| {
                    PackageRelationship::try_from(r.trim())
                        .map_err(|_| de::Error::invalid_value(de::Unexpected::Str(r), &self))
                })
                .collect()
        }
    }

    deserializer.deserialize_str(Visitor {})
}

#[derive(Deserialize, Debug, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
struct BinaryPackage {
    #[serde(flatten)]
    package: source_packages::BinaryPackage,
    architecture: Architecture,
    #[serde(
        rename = "Built-Using",
        default,
        deserialize_with = "deserialize_package_relationships"
    )]
    built_using: Vec<PackageRelationship>,
    #[serde(
        rename = "Static-Built-Using",
        default,
        deserialize_with = "deserialize_package_relationships"
    )]
    static_built_using: Vec<PackageRelationship>,
    #[serde(
        rename = "X-Cargo-Built-Using",
        default,
        deserialize_with = "deserialize_package_relationships"
    )]
    x_cargo_built_using: Vec<PackageRelationship>,
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
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CombinedOutdatedPackage {
    fn cmp(&self, other: &Self) -> Ordering {
        self.source.cmp(&other.source)
    }
}

struct BinaryPackageParser<'a> {
    fields: &'a [Field],
    iterator: ProgressBarIter<IntoIter<BinaryPackage>>,
    sources: &'a SourcePackages,
}

impl<'a> BinaryPackageParser<'a> {
    fn new<P>(fields: &'a [Field], sources: &'a SourcePackages, path: P) -> Result<Self>
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
            fields,
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

            let source_package = binary_package.package.source_package();
            let built_using_set: HashSet<_> = self
                .fields
                .iter()
                .filter_map(|field| {
                    // skip packages without Built-Using
                    let built_using = match field {
                        Field::BuiltUsing => &binary_package.built_using,
                        Field::StaticBuiltUsing => &binary_package.static_built_using,
                        Field::XCargoBuiltUsing => &binary_package.x_cargo_built_using,
                    };
                    if built_using.is_empty() {
                        None
                    } else {
                        Some(built_using)
                    }
                })
                .flat_map(|built_using| {
                    built_using
                        .iter()
                        .filter_map(|dependency| {
                            let PackageRelationship {
                                package,
                                version_relation,
                                architecture_restrictions,
                                build_profiles,
                            } = dependency;
                            match (version_relation, architecture_restrictions, build_profiles) {
                                (
                                    Some(VersionRelationship {
                                        version,
                                        relation: Relationship::Equal,
                                    }),
                                    None,
                                    None,
                                ) => Some(VersionedPackage {
                                    package: package.clone(),
                                    version: version.clone(),
                                }),
                                _ => {
                                    warn!(
                                        "Package '{}' contains invalid dependency: {}",
                                        binary_package.package.package, dependency
                                    );
                                    None
                                }
                            }
                        })
                        .filter(|source| {
                            self.sources
                                .version(&source.package)
                                .map(|current_version| source.version < *current_version)
                                .unwrap_or(true)
                        })
                })
                .collect();
            // all packages in Built-Using are up to date
            if built_using_set.is_empty() {
                trace!(
                    "Skipping {}: all dependencies are up-to-date.",
                    source_package.package
                );
                continue;
            }

            // if the package builds any MA: same packages, schedule binNMUs with ANY
            let architecture = if self.sources.is_ma_same(&source_package.package) {
                WBArchitecture::Any
            } else {
                WBArchitecture::Architecture(binary_package.architecture)
            };
            return Some(OutdatedSourcePackage {
                source: source_package,
                built_using: built_using_set,
                architecture,
            });
        }

        None
    }
}

impl FusedIterator for BinaryPackageParser<'_> {}

trait LoadUDDBugs {
    fn load_bugs(&self, suite: SuiteOrCodename) -> Result<UDDBugs>;
}

pub(crate) struct NMUOutdatedBuiltUsing<'a, C>
where
    C: CachePaths,
{
    cache: &'a C,
    base_options: &'a BaseOptions,
    options: NMUOutdatedBuiltUsingOptions,
}

impl<'a, C> NMUOutdatedBuiltUsing<'a, C>
where
    C: CachePaths,
{
    pub(crate) fn new(
        cache: &'a C,
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
        fields: &[Field],
        suite: SuiteOrCodename,
        source_packages: &SourcePackages,
    ) -> Result<Vec<CombinedOutdatedPackage>>
    where
        Self: LoadUDDBugs,
    {
        let ftbfs_bugs = self.load_bugs(suite)?;

        // collect outdated binary packages
        let mut packages = HashSet::new();
        for suite in self.expand_suite_for_binaries() {
            let converted_suite = suite.into();
            for path in self.cache.get_package_paths(suite, false)? {
                for OutdatedSourcePackage {
                    source,
                    built_using: dependencies,
                    architecture,
                } in BinaryPackageParser::new(fields, source_packages, path)?
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

        let mut result = HashMap::<_, HashSet<(PackageVersion, VersionedPackage)>>::new();
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

    fn generate_wb_commands(&self) -> Result<Vec<WBCommand>>
    where
        Self: LoadUDDBugs,
    {
        let source_packages = self.load_sources_for_suites(&self.expand_suite_for_sources())?;
        let fields = if self.options.field.is_empty() {
            [Field::BuiltUsing].as_ref()
        } else {
            self.options.field.as_ref()
        };
        let display_field = fields.iter().join("/");
        let eso_sources = self.load_eso(fields, self.options.suite, &source_packages)?;

        let mut wb_commands = Vec::new();
        for outdated_package in eso_sources {
            let mut source = SourceSpecifier::new(&outdated_package.source.package);
            source.with_version(&outdated_package.source.version);
            source.with_suite(outdated_package.suite.into());
            source.with_architectures(&[outdated_package.architecture]);

            let message = format!(
                "Rebuild for outdated {} ({})",
                display_field,
                outdated_package
                    .outdated_dependencies
                    .iter()
                    .map(|source| format!("{}/{}", source.package, source.version))
                    .join(", ")
            );
            let mut binnmu = BinNMU::new(&source, &message)?;
            binnmu.with_build_priority(self.options.build_priority);

            let mut extra_depends = Vec::new();
            for outdated_dependency in &outdated_package.outdated_dependencies {
                if let Ok(required_dependency) = REQUIRES_EXTRA_DEPENDS
                    .binary_search_by(|extra_depends| {
                        extra_depends
                            .source
                            .cmp(outdated_dependency.package.as_ref())
                    })
                    .map(|index| &REQUIRES_EXTRA_DEPENDS[index])
                    && let Some(version) = source_packages.version(required_dependency.source)
                {
                    extra_depends.push(PackageRelationship {
                        package: PackageName::try_from(required_dependency.package).unwrap(),
                        version_relation: Some(VersionRelationship {
                            relation: Relationship::GreaterEqual,
                            version: version.clone(),
                        }),
                        architecture_restrictions: None,
                        build_profiles: None,
                    });
                }
            }
            if !extra_depends.is_empty() {
                binnmu.with_extra_depends(&extra_depends);
            }

            wb_commands.push(binnmu.build());
        }

        Ok(wb_commands)
    }
}

impl LoadUDDBugs for NMUOutdatedBuiltUsing<'_, Cache> {
    fn load_bugs(&self, suite: SuiteOrCodename) -> Result<UDDBugs> {
        UDDBugs::load_for_codename(self.cache, suite)
    }
}

#[async_trait]
impl AsyncCommand for NMUOutdatedBuiltUsing<'_, Cache> {
    async fn run(&self) -> Result<()> {
        let wb_commands = self.generate_wb_commands()?;
        execute_wb_commands(wb_commands, self.base_options).await
    }
}

impl Downloads for NMUOutdatedBuiltUsing<'_, Cache> {
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

struct RequiresExtraDepends {
    source: &'static str,
    package: &'static str,
}

// this array needs to be sorted by source
const REQUIRES_EXTRA_DEPENDS: [RequiresExtraDepends; 4] = [
    RequiresExtraDepends {
        source: "binutils",
        package: "binutils",
    },
    RequiresExtraDepends {
        source: "dpkg",
        package: "dpkg-dev",
    },
    RequiresExtraDepends {
        source: "glibc",
        package: "libc-bin",
    },
    RequiresExtraDepends {
        source: "perl",
        package: "perl-base",
    },
];

#[cfg(test)]
mod test {
    use std::{fs::File, io::Write, path::PathBuf};

    use clap_verbosity_flag::Verbosity;
    use tempfile::tempdir;

    use super::*;

    struct TestCache {
        base_dir: PathBuf,
    }

    impl CachePaths for TestCache {
        fn get_cache_path<P>(&self, path: P) -> Result<PathBuf>
        where
            P: AsRef<Path>,
        {
            Ok(self.base_dir.join(path))
        }

        fn get_package_paths(&self, suite: SuiteOrCodename, _: bool) -> Result<Vec<PathBuf>> {
            Ok(vec![self.get_package_path(suite, Architecture::Amd64)?])
        }
    }

    impl LoadUDDBugs for NMUOutdatedBuiltUsing<'_, TestCache> {
        fn load_bugs(&self, _: SuiteOrCodename) -> Result<UDDBugs> {
            Ok(UDDBugs::default())
        }
    }

    #[test]
    fn base() {
        let base_options = BaseOptions {
            force_download: false,
            force_processing: true,
            dry_run: true,
            verbose: Verbosity::new(0, 1),
            buildd: String::new(),
            mirror: String::new(),
        };
        let options = NMUOutdatedBuiltUsingOptions {
            build_priority: 0,
            suite: SuiteOrCodename::UNSTABLE,
            field: vec![Field::BuiltUsing],
        };

        let temp_dir = tempdir().unwrap();
        {
            let mut packages =
                File::create(temp_dir.path().join("Packages_unstable_amd64")).unwrap();
            writeln!(
                packages,
                r"Package: acmetool
Source: acmetool (0.2.2-3)
Version: 0.2.2-3+b1
Installed-Size: 11859
Maintainer: Debian Go Packaging Team <pkg-go-maintainers@lists.alioth.debian.org>
Architecture: amd64
Depends: libc6 (>= 2.34), libcap2 (>= 1:2.10)
Recommends: dialog
Description: automatic certificate acquisition tool for Let's Encrypt
Built-Using: golang-1.24 (= 1.24.4-1)
Description-md5: 3e5e145ae880b97f3b6e825daf35ce32
Section: web
Priority: optional
Filename: pool/main/a/acmetool/acmetool_0.2.2-3+b1_amd64.deb
Size: 3629452
MD5sum: 10ca3f82368c7d166cbac9ecc6db9117
SHA256: 8f8dc696ef02e3b9cf571ff15a7a2e5086c0c10e8088f2f11e20c442fac5d446
"
            )
            .unwrap();
            packages.flush().unwrap();

            let mut sources = File::create(temp_dir.path().join("Sources_unstable")).unwrap();
            writeln!(
                sources,
                r"Package: acmetool
Binary: acmetool
Version: 0.2.2-3
Maintainer: Debian Go Packaging Team <pkg-go-maintainers@lists.alioth.debian.org>
Uploaders: Peter Colberg <peter@colberg.org>,
Build-Depends: debhelper-compat (= 13), dh-apache2, dh-golang
Architecture: any
Standards-Version: 4.6.2
Format: 3.0 (quilt)
Files:
 1481ab6356d2e63bf378fe3f96cf5b8e 2697 acmetool_0.2.2-3.dsc
 9d21da41c887cb669479b4eb3b1e08b7 121583 acmetool_0.2.2.orig.tar.gz
 58040de8ffdf39685ce25f468773990c 10012 acmetool_0.2.2-3.debian.tar.xz
Checksums-Sha256:
 15a995d1879ac58233a9fbff565451c3883342fe4361ee2046b2ca765f6aeaef 2697 acmetool_0.2.2-3.dsc
 5671a4ff00c007dd00883c601c0a64ab9c4dc1ca4fa47e5801b69b015d43dfb3 121583 acmetool_0.2.2.orig.tar.gz
 cbf53556dbf1cc042e5f3f5633d2b93f49da29210482c563c3286cdc1594e455 10012 acmetool_0.2.2-3.debian.tar.xz
Build-Depends-Arch: golang-any, golang-github-coreos-go-systemd-dev, golang-github-gofrs-uuid-dev, golang-github-hlandau-dexlogconfig-dev, golang-github-hlandau-goutils-dev, golang-github-hlandau-xlog-dev, golang-github-jmhodges-clock-dev, golang-github-mitchellh-go-wordwrap-dev, golang-golang-x-net-dev, golang-gopkg-alecthomas-kingpin.v2-dev, golang-gopkg-cheggaaa-pb.v1-dev, golang-gopkg-hlandau-acmeapi.v2-dev, golang-gopkg-hlandau-easyconfig.v1-dev, golang-gopkg-hlandau-service.v2-dev, golang-gopkg-hlandau-svcutils.v1-dev, golang-gopkg-square-go-jose.v1-dev, golang-gopkg-tylerb-graceful.v1-dev, golang-gopkg-yaml.v2-dev | golang-yaml.v2-dev, libcap-dev [linux-any]
Go-Import-Path: github.com/hlandau/acmetool
Package-List: 
 acmetool deb web optional arch=any
Testsuite: autopkgtest-pkg-go
Directory: pool/main/a/acmetool
Priority: optional
Section: misc

Package: golang-1.24
Binary: golang-1.24-go, golang-1.24-src, golang-1.24-doc, golang-1.24
Version: 1.24.4-1
Maintainer: Debian Go Compiler Team <team+go-compiler@tracker.debian.org>
Uploaders: Michael Stapelberg <stapelberg@debian.org>, Paul Tagliamonte <paultag@debian.org>, Tianon Gravi <tianon@debian.org>, Michael Hudson-Doyle <mwhudson@debian.org>, Anthony Fok <foka@debian.org>
Build-Depends: debhelper-compat (= 13), binutils-gold [arm64], golang-1.24-go | golang-1.23-go | golang-1.22-go, netbase
Architecture: amd64 arm64 armel armhf i386 loong64 mips mips64el mipsel ppc64 ppc64el riscv64 s390x all
Standards-Version: 4.6.2
Format: 3.0 (quilt)
Files:
 ad3a8c72ddf40d2134bda9c4177a84cf 2877 golang-1.24_1.24.4-1.dsc
 38d0b0a73d5b1b174e3a23be17fa10a0 30788576 golang-1.24_1.24.4.orig.tar.gz
 07c6573541a198828d75a04250c86946 833 golang-1.24_1.24.4.orig.tar.gz.asc
 d00ba8b8423714cb16788465514da2a1 42192 golang-1.24_1.24.4-1.debian.tar.xz
Checksums-Sha256:
 f9991da1d502c1dd278f236f5cb960b7f0113e68cfe3739427aeb58853afbde3 2877 golang-1.24_1.24.4-1.dsc
 5a86a83a31f9fa81490b8c5420ac384fd3d95a3e71fba665c7b3f95d1dfef2b4 30788576 golang-1.24_1.24.4.orig.tar.gz
 bcc618ca95f9da9870907c265f9e12aef2ca6e37612a8d15d37ecbc828c420f6 833 golang-1.24_1.24.4.orig.tar.gz.asc
 b613c9f5f2a4179ea618854e4310422231f115bab97cc5c18707a720d612da32 42192 golang-1.24_1.24.4-1.debian.tar.xz
Package-List: 
 golang-1.24 deb golang optional arch=all
 golang-1.24-doc deb doc optional arch=all
 golang-1.24-go deb golang optional arch=amd64,arm64,armel,armhf,i386,loong64,mips,mips64el,mipsel,ppc64,ppc64el,riscv64,s390x
 golang-1.24-src deb golang optional arch=all
Testsuite: autopkgtest
Testsuite-Triggers: build-essential
Extra-Source-Only: yes
Directory: pool/main/g/golang-1.24
Priority: optional
Section: misc

Package: golang-1.24
Binary: golang-1.24-go, golang-1.24-src, golang-1.24-doc, golang-1.24
Version: 1.24.9-1
Maintainer: Debian Go Compiler Team <team+go-compiler@tracker.debian.org>
Uploaders: Michael Stapelberg <stapelberg@debian.org>, Paul Tagliamonte <paultag@debian.org>, Tianon Gravi <tianon@debian.org>, Michael Hudson-Doyle <mwhudson@debian.org>, Anthony Fok <foka@debian.org>
Build-Depends: debhelper-compat (= 13), binutils-gold [arm64], golang-1.24-go:native | golang-1.23-go:native | golang-1.22-go:native, netbase
Architecture: amd64 arm64 armel armhf i386 loong64 mips mips64el mipsel ppc64 ppc64el riscv64 s390x all
Standards-Version: 4.6.2
Format: 3.0 (quilt)
Files:
 1892eeaec73cba64c144ed56a4dbe42a 2923 golang-1.24_1.24.9-1.dsc
 5c2c3969fddd1b8d320dc06fcf705732 30800154 golang-1.24_1.24.9.orig.tar.gz
 96b578fe4cd5c58b53b71743be91641f 833 golang-1.24_1.24.9.orig.tar.gz.asc
 99c340f1841007eb4695ebedcb8489e2 44808 golang-1.24_1.24.9-1.debian.tar.xz
Checksums-Sha256:
 067677ffb7c04162ae5412ccaac2a31d6e60716386f55652fb1c8a18c8e121d0 2923 golang-1.24_1.24.9-1.dsc
 c72f81ba54fe00efe7f3e7499d400979246881b13b775e9a9bb85541c11be695 30800154 golang-1.24_1.24.9.orig.tar.gz
 23fbe2d3a664451d901aa3681889ec3603c5a65b1dfd8655119e08d592433904 833 golang-1.24_1.24.9.orig.tar.gz.asc
 d21ee50c57bb1a759568d26662bc310249e643261ea00fc979362d903bdc10bd 44808 golang-1.24_1.24.9-1.debian.tar.xz
Package-List: 
 golang-1.24 deb golang optional arch=all
 golang-1.24-doc deb doc optional arch=all
 golang-1.24-go deb golang optional arch=amd64,arm64,armel,armhf,i386,loong64,mips,mips64el,mipsel,ppc64,ppc64el,riscv64,s390x
 golang-1.24-src deb golang optional arch=all
Testsuite: autopkgtest
Testsuite-Triggers: build-essential
Directory: pool/main/g/golang-1.24
Priority: optional
Section: misc
"
            ).unwrap();
            sources.flush().unwrap();
        }

        let cache = TestCache {
            base_dir: temp_dir.path().into(),
        };

        let nmu_eso = NMUOutdatedBuiltUsing::new(&cache, &base_options, options);
        let wb_commands = nmu_eso.generate_wb_commands().unwrap();
        assert_eq!(wb_commands.len(), 1);
    }

    #[test]
    fn only_eso() {
        let base_options = BaseOptions {
            force_download: false,
            force_processing: true,
            dry_run: true,
            verbose: Verbosity::new(0, 1),
            buildd: String::new(),
            mirror: String::new(),
        };
        let options = NMUOutdatedBuiltUsingOptions {
            build_priority: 0,
            suite: SuiteOrCodename::UNSTABLE,
            field: vec![Field::BuiltUsing],
        };

        let temp_dir = tempdir().unwrap();
        {
            let mut packages =
                File::create(temp_dir.path().join("Packages_unstable_amd64")).unwrap();
            writeln!(
                packages,
                r"Package: acmetool
Source: acmetool (0.2.2-3)
Version: 0.2.2-3+b1
Installed-Size: 11859
Maintainer: Debian Go Packaging Team <pkg-go-maintainers@lists.alioth.debian.org>
Architecture: amd64
Depends: libc6 (>= 2.34), libcap2 (>= 1:2.10)
Recommends: dialog
Description: automatic certificate acquisition tool for Let's Encrypt
Built-Using: golang-1.24 (= 1.24.4-1)
Description-md5: 3e5e145ae880b97f3b6e825daf35ce32
Section: web
Priority: optional
Filename: pool/main/a/acmetool/acmetool_0.2.2-3+b1_amd64.deb
Size: 3629452
MD5sum: 10ca3f82368c7d166cbac9ecc6db9117
SHA256: 8f8dc696ef02e3b9cf571ff15a7a2e5086c0c10e8088f2f11e20c442fac5d446
"
            )
            .unwrap();
            packages.flush().unwrap();

            let mut sources = File::create(temp_dir.path().join("Sources_unstable")).unwrap();
            writeln!(
                sources,
                r"Package: acmetool
Binary: acmetool
Version: 0.2.2-3
Maintainer: Debian Go Packaging Team <pkg-go-maintainers@lists.alioth.debian.org>
Uploaders: Peter Colberg <peter@colberg.org>,
Build-Depends: debhelper-compat (= 13), dh-apache2, dh-golang
Architecture: any
Standards-Version: 4.6.2
Format: 3.0 (quilt)
Files:
 1481ab6356d2e63bf378fe3f96cf5b8e 2697 acmetool_0.2.2-3.dsc
 9d21da41c887cb669479b4eb3b1e08b7 121583 acmetool_0.2.2.orig.tar.gz
 58040de8ffdf39685ce25f468773990c 10012 acmetool_0.2.2-3.debian.tar.xz
Checksums-Sha256:
 15a995d1879ac58233a9fbff565451c3883342fe4361ee2046b2ca765f6aeaef 2697 acmetool_0.2.2-3.dsc
 5671a4ff00c007dd00883c601c0a64ab9c4dc1ca4fa47e5801b69b015d43dfb3 121583 acmetool_0.2.2.orig.tar.gz
 cbf53556dbf1cc042e5f3f5633d2b93f49da29210482c563c3286cdc1594e455 10012 acmetool_0.2.2-3.debian.tar.xz
Build-Depends-Arch: golang-any, golang-github-coreos-go-systemd-dev, golang-github-gofrs-uuid-dev, golang-github-hlandau-dexlogconfig-dev, golang-github-hlandau-goutils-dev, golang-github-hlandau-xlog-dev, golang-github-jmhodges-clock-dev, golang-github-mitchellh-go-wordwrap-dev, golang-golang-x-net-dev, golang-gopkg-alecthomas-kingpin.v2-dev, golang-gopkg-cheggaaa-pb.v1-dev, golang-gopkg-hlandau-acmeapi.v2-dev, golang-gopkg-hlandau-easyconfig.v1-dev, golang-gopkg-hlandau-service.v2-dev, golang-gopkg-hlandau-svcutils.v1-dev, golang-gopkg-square-go-jose.v1-dev, golang-gopkg-tylerb-graceful.v1-dev, golang-gopkg-yaml.v2-dev | golang-yaml.v2-dev, libcap-dev [linux-any]
Go-Import-Path: github.com/hlandau/acmetool
Package-List: 
 acmetool deb web optional arch=any
Testsuite: autopkgtest-pkg-go
Directory: pool/main/a/acmetool
Priority: optional
Section: misc

Package: golang-1.24
Binary: golang-1.24-go, golang-1.24-src, golang-1.24-doc, golang-1.24
Version: 1.24.4-1
Maintainer: Debian Go Compiler Team <team+go-compiler@tracker.debian.org>
Uploaders: Michael Stapelberg <stapelberg@debian.org>, Paul Tagliamonte <paultag@debian.org>, Tianon Gravi <tianon@debian.org>, Michael Hudson-Doyle <mwhudson@debian.org>, Anthony Fok <foka@debian.org>
Build-Depends: debhelper-compat (= 13), binutils-gold [arm64], golang-1.24-go | golang-1.23-go | golang-1.22-go, netbase
Architecture: amd64 arm64 armel armhf i386 loong64 mips mips64el mipsel ppc64 ppc64el riscv64 s390x all
Standards-Version: 4.6.2
Format: 3.0 (quilt)
Files:
 ad3a8c72ddf40d2134bda9c4177a84cf 2877 golang-1.24_1.24.4-1.dsc
 38d0b0a73d5b1b174e3a23be17fa10a0 30788576 golang-1.24_1.24.4.orig.tar.gz
 07c6573541a198828d75a04250c86946 833 golang-1.24_1.24.4.orig.tar.gz.asc
 d00ba8b8423714cb16788465514da2a1 42192 golang-1.24_1.24.4-1.debian.tar.xz
Checksums-Sha256:
 f9991da1d502c1dd278f236f5cb960b7f0113e68cfe3739427aeb58853afbde3 2877 golang-1.24_1.24.4-1.dsc
 5a86a83a31f9fa81490b8c5420ac384fd3d95a3e71fba665c7b3f95d1dfef2b4 30788576 golang-1.24_1.24.4.orig.tar.gz
 bcc618ca95f9da9870907c265f9e12aef2ca6e37612a8d15d37ecbc828c420f6 833 golang-1.24_1.24.4.orig.tar.gz.asc
 b613c9f5f2a4179ea618854e4310422231f115bab97cc5c18707a720d612da32 42192 golang-1.24_1.24.4-1.debian.tar.xz
Package-List: 
 golang-1.24 deb golang optional arch=all
 golang-1.24-doc deb doc optional arch=all
 golang-1.24-go deb golang optional arch=amd64,arm64,armel,armhf,i386,loong64,mips,mips64el,mipsel,ppc64,ppc64el,riscv64,s390x
 golang-1.24-src deb golang optional arch=all
Testsuite: autopkgtest
Testsuite-Triggers: build-essential
Extra-Source-Only: yes
Directory: pool/main/g/golang-1.24
Priority: optional
Section: misc
"
            ).unwrap();
            sources.flush().unwrap();
        }

        let cache = TestCache {
            base_dir: temp_dir.path().into(),
        };

        let nmu_eso = NMUOutdatedBuiltUsing::new(&cache, &base_options, options);
        let wb_commands = nmu_eso.generate_wb_commands().unwrap();
        assert_eq!(wb_commands.len(), 1);
    }

    #[test]
    fn non_existing_source() {
        let base_options = BaseOptions {
            force_download: false,
            force_processing: true,
            dry_run: true,
            verbose: Verbosity::new(0, 1),
            buildd: String::new(),
            mirror: String::new(),
        };
        let options = NMUOutdatedBuiltUsingOptions {
            build_priority: 0,
            suite: SuiteOrCodename::UNSTABLE,
            field: vec![Field::BuiltUsing],
        };

        let temp_dir = tempdir().unwrap();
        {
            let mut packages =
                File::create(temp_dir.path().join("Packages_unstable_amd64")).unwrap();
            writeln!(
                packages,
                r"Package: acmetool
Source: acmetool (0.2.2-3)
Version: 0.2.2-3+b1
Installed-Size: 11859
Maintainer: Debian Go Packaging Team <pkg-go-maintainers@lists.alioth.debian.org>
Architecture: amd64
Depends: libc6 (>= 2.34), libcap2 (>= 1:2.10)
Recommends: dialog
Description: automatic certificate acquisition tool for Let's Encrypt
Built-Using: golang-1.24 (= 1.24.4-1)
Description-md5: 3e5e145ae880b97f3b6e825daf35ce32
Section: web
Priority: optional
Filename: pool/main/a/acmetool/acmetool_0.2.2-3+b1_amd64.deb
Size: 3629452
MD5sum: 10ca3f82368c7d166cbac9ecc6db9117
SHA256: 8f8dc696ef02e3b9cf571ff15a7a2e5086c0c10e8088f2f11e20c442fac5d446
"
            )
            .unwrap();
            packages.flush().unwrap();

            let mut sources = File::create(temp_dir.path().join("Sources_unstable")).unwrap();
            writeln!(
                sources,
                r"Package: acmetool
Binary: acmetool
Version: 0.2.2-3
Maintainer: Debian Go Packaging Team <pkg-go-maintainers@lists.alioth.debian.org>
Uploaders: Peter Colberg <peter@colberg.org>,
Build-Depends: debhelper-compat (= 13), dh-apache2, dh-golang
Architecture: any
Standards-Version: 4.6.2
Format: 3.0 (quilt)
Files:
 1481ab6356d2e63bf378fe3f96cf5b8e 2697 acmetool_0.2.2-3.dsc
 9d21da41c887cb669479b4eb3b1e08b7 121583 acmetool_0.2.2.orig.tar.gz
 58040de8ffdf39685ce25f468773990c 10012 acmetool_0.2.2-3.debian.tar.xz
Checksums-Sha256:
 15a995d1879ac58233a9fbff565451c3883342fe4361ee2046b2ca765f6aeaef 2697 acmetool_0.2.2-3.dsc
 5671a4ff00c007dd00883c601c0a64ab9c4dc1ca4fa47e5801b69b015d43dfb3 121583 acmetool_0.2.2.orig.tar.gz
 cbf53556dbf1cc042e5f3f5633d2b93f49da29210482c563c3286cdc1594e455 10012 acmetool_0.2.2-3.debian.tar.xz
Build-Depends-Arch: golang-any, golang-github-coreos-go-systemd-dev, golang-github-gofrs-uuid-dev, golang-github-hlandau-dexlogconfig-dev, golang-github-hlandau-goutils-dev, golang-github-hlandau-xlog-dev, golang-github-jmhodges-clock-dev, golang-github-mitchellh-go-wordwrap-dev, golang-golang-x-net-dev, golang-gopkg-alecthomas-kingpin.v2-dev, golang-gopkg-cheggaaa-pb.v1-dev, golang-gopkg-hlandau-acmeapi.v2-dev, golang-gopkg-hlandau-easyconfig.v1-dev, golang-gopkg-hlandau-service.v2-dev, golang-gopkg-hlandau-svcutils.v1-dev, golang-gopkg-square-go-jose.v1-dev, golang-gopkg-tylerb-graceful.v1-dev, golang-gopkg-yaml.v2-dev | golang-yaml.v2-dev, libcap-dev [linux-any]
Go-Import-Path: github.com/hlandau/acmetool
Package-List: 
 acmetool deb web optional arch=any
Testsuite: autopkgtest-pkg-go
Directory: pool/main/a/acmetool
Priority: optional
Section: misc
"
            ).unwrap();
            sources.flush().unwrap();
        }

        let cache = TestCache {
            base_dir: temp_dir.path().into(),
        };

        let nmu_eso = NMUOutdatedBuiltUsing::new(&cache, &base_options, options);
        let wb_commands = nmu_eso.generate_wb_commands().unwrap();
        assert_eq!(wb_commands.len(), 1);
    }

    #[test]
    fn required_extra_depends_is_sorted() {
        assert!(
            REQUIRES_EXTRA_DEPENDS
                .iter()
                .map(|red| red.source)
                .is_sorted()
        );
    }
}
