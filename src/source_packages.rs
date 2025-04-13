// Copyright 2021-2025 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    borrow::Borrow,
    collections::HashMap,
    fmt::{self, Display},
    hash::Hash,
    path::Path,
};

use anyhow::Result;
use assorted_debian_utils::{
    archive::MultiArch, package::PackageName, rfc822_like, version::PackageVersion,
};
use indicatif::{ProgressBar, ProgressIterator};
use serde::{
    Deserialize,
    de::{self, DeserializeOwned},
};

use crate::config;

/// Source package name with optional version in parenthesis
#[derive(Debug, PartialEq, Eq)]
pub struct SourceWithVersion {
    pub source: PackageName,
    pub version: Option<PackageVersion>,
}

impl Display for SourceWithVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref version) = self.version {
            write!(f, "{} ({})", self.source, version)
        } else {
            write!(f, "{}", self.source)
        }
    }
}

impl<'de> Deserialize<'de> for SourceWithVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor;

        impl de::Visitor<'_> for Visitor {
            type Value = SourceWithVersion;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(
                    formatter,
                    "a source package name or a source package name with version formatted as $source ($version)"
                )
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let (source, version) = if let Some((source, version)) = v.split_once(' ') {
                    if !version.starts_with('(') || !version.ends_with(')') {
                        return Err(E::invalid_value(de::Unexpected::Str(v), &self));
                    }

                    let version = PackageVersion::try_from(&version[1..version.len() - 1])
                        .map_err(|_| E::invalid_value(de::Unexpected::Str(v), &self))?;
                    (source, Some(version))
                } else {
                    (v, None)
                };

                source
                    .try_into()
                    .map_err(|_| E::invalid_value(de::Unexpected::Str(v), &self))
                    .map(|source| SourceWithVersion { source, version })
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub struct BinaryPackage {
    pub source: Option<SourceWithVersion>,
    pub package: PackageName,
    pub version: PackageVersion,
    #[serde(rename = "Multi-Arch")]
    pub multi_arch: Option<MultiArch>,
}

impl BinaryPackage {
    pub fn name_and_version(&self) -> (&PackageName, PackageVersion) {
        if let Some(source_package) = &self.source {
            (
                &source_package.source,
                source_package
                    .version
                    .clone()
                    .unwrap_or_else(|| self.version.clone_without_binnmu_version()),
            )
        } else {
            // no Source set, so Source == Package
            (&self.package, self.version.clone_without_binnmu_version())
        }
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
struct SourcePackage {
    package: PackageName,
    version: PackageVersion,
}

#[derive(Debug)]
struct SourcePackageInfo {
    ma_same: bool,
    version: PackageVersion,
}

pub struct SourcePackages(HashMap<PackageName, SourcePackageInfo>);

impl SourcePackages {
    /// Extract source package information from binary package files
    ///
    /// This step needs to be performed from binary packages to check whether a
    /// source package builds MA: same binary packages.
    pub fn new<P>(paths: &[P]) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let mut all_sources = HashMap::<PackageName, SourcePackageInfo>::new();
        for path in paths {
            for binary_package in parse_packages::<BinaryPackage>(path.as_ref())? {
                let (source, version) = binary_package.name_and_version();

                if let Some(data) = all_sources.get_mut(source) {
                    // store only highest version
                    if version > data.version {
                        data.version = version;
                    }
                    if !data.ma_same && binary_package.multi_arch == Some(MultiArch::Same) {
                        data.ma_same = true
                    }
                } else {
                    all_sources.insert(
                        source.clone(),
                        SourcePackageInfo {
                            version,
                            ma_same: binary_package.multi_arch == Some(MultiArch::Same),
                        },
                    );
                }
            }
        }

        Ok(Self(all_sources))
    }

    /// Extract source package information from binary package files
    ///
    /// This step needs to be performed from binary packages to check whether a
    /// source package builds MA: same binary packages.
    pub fn new_with_source<P, Q>(sources: &[P], paths: &[Q]) -> Result<Self>
    where
        P: AsRef<Path>,
        Q: AsRef<Path>,
    {
        let mut all_sources = HashMap::<PackageName, SourcePackageInfo>::new();
        for path in sources {
            for source_package in parse_packages::<SourcePackage>(path.as_ref())? {
                if let Some(data) = all_sources.get_mut(&source_package.package) {
                    // store only highest version
                    if source_package.version > data.version {
                        data.version = source_package.version;
                    }
                } else {
                    all_sources.insert(
                        source_package.package,
                        SourcePackageInfo {
                            version: source_package.version,
                            ma_same: false,
                        },
                    );
                }
            }
        }

        for path in paths {
            for binary_package in parse_packages::<BinaryPackage>(path.as_ref())? {
                let (source, version) = binary_package.name_and_version();

                if let Some(data) = all_sources.get_mut(source) {
                    // store only highest version
                    if version > data.version {
                        data.version = version;
                    }
                    if !data.ma_same && binary_package.multi_arch == Some(MultiArch::Same) {
                        data.ma_same = true
                    }
                } else {
                    all_sources.insert(
                        source.clone(),
                        SourcePackageInfo {
                            version,
                            ma_same: binary_package.multi_arch == Some(MultiArch::Same),
                        },
                    );
                }
            }
        }

        Ok(Self(all_sources))
    }

    /// Check if a source package builds an MA: same binary package
    ///
    /// Returns false if the source package does not exist.
    pub fn is_ma_same<Q>(&self, source: &Q) -> bool
    where
        Q: ?Sized + Hash + Eq,
        PackageName: Borrow<Q>,
    {
        self.0
            .get(source)
            .map(|source_package| source_package.ma_same)
            .unwrap_or_default()
    }

    /// Get the maximal version of a source package
    pub fn version<Q>(&self, source: &Q) -> Option<&PackageVersion>
    where
        Q: ?Sized + Hash + Eq,
        PackageName: Borrow<Q>,
    {
        self.0
            .get(source)
            .map(|source_package| &source_package.version)
    }
}

fn parse_packages<P>(path: &Path) -> Result<impl Iterator<Item = P>>
where
    P: DeserializeOwned,
{
    // read Package file
    let binary_packages: Vec<P> = rfc822_like::from_file(path)?;
    let pb = ProgressBar::new(binary_packages.len() as u64);
    pb.set_style(config::default_progress_style().template(
        "{msg}: {spinner:.green} [{wide_bar:.cyan/blue}] {pos}/{len} ({per_sec}, {eta})",
    )?);
    pb.set_message(format!("Processing {}", path.display()));
    // collect all sources
    Ok(binary_packages.into_iter().progress_with(pb))
}
