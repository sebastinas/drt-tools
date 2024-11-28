// Copyright 2021-2024 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use assorted_debian_utils::{archive::MultiArch, rfc822_like, version::PackageVersion};
use indicatif::{ProgressBar, ProgressIterator};
use serde::Deserialize;

use crate::config;

#[derive(Deserialize, Debug, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
struct BinaryPackage {
    source: Option<String>,
    package: String,
    version: PackageVersion,
    #[serde(rename = "Multi-Arch")]
    multi_arch: Option<MultiArch>,
}

#[derive(Debug)]
struct SourcePackage {
    ma_same: bool,
    version: PackageVersion,
}

pub struct SourcePackages(HashMap<String, SourcePackage>);

impl SourcePackages {
    pub fn new<P>(paths: &[P]) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let mut all_sources = HashMap::<String, SourcePackage>::new();
        for path in paths {
            for binary_package in Self::parse_packages(path)? {
                let (source, version) = if let Some(source_package) = &binary_package.source {
                    source_package
                        .split_once(|c: char| c.is_ascii_whitespace())
                        .map(|(source, version)| {
                            (
                                source.into(),
                                PackageVersion::try_from(&version[1..version.len() - 1]).unwrap(),
                            )
                        })
                        .unwrap_or_else(|| {
                            (
                                source_package.into(),
                                binary_package.version.without_binnmu_version(),
                            )
                        })
                } else {
                    // no Source set, so Source == Package
                    (
                        binary_package.package,
                        binary_package.version.without_binnmu_version(),
                    )
                };

                if let Some(data) = all_sources.get_mut(&source) {
                    if version > data.version {
                        data.version = version;
                    }
                    if !data.ma_same && binary_package.multi_arch == Some(MultiArch::Same) {
                        data.ma_same = true
                    }
                } else {
                    all_sources.insert(
                        source,
                        SourcePackage {
                            version,
                            ma_same: binary_package.multi_arch == Some(MultiArch::Same),
                        },
                    );
                }
            }
        }

        Ok(Self(all_sources))
    }

    fn parse_packages<P>(path: P) -> Result<impl Iterator<Item = BinaryPackage>>
    where
        P: AsRef<Path>,
    {
        // read Package file
        let binary_packages: Vec<BinaryPackage> = rfc822_like::from_file(path.as_ref())?;
        let pb = ProgressBar::new(binary_packages.len() as u64);
        pb.set_style(config::default_progress_style().template(
            "{msg}: {spinner:.green} [{wide_bar:.cyan/blue}] {pos}/{len} ({per_sec}, {eta})",
        )?);
        pb.set_message(format!("Processing {}", path.as_ref().display()));
        // collect all sources
        Ok(binary_packages.into_iter().progress_with(pb))
    }

    /// Check if a source package builds an MA: same binary package
    ///
    /// Returns false if the source package does not exist.
    pub fn is_ma_same(&self, source: &str) -> bool {
        self.0
            .get(source)
            .map(|source_package| source_package.ma_same)
            .unwrap_or_default()
    }

    /// Get the maximal version of a source package
    pub fn version(&self, source: &str) -> Option<&PackageVersion> {
        self.0
            .get(source)
            .map(|source_package| &source_package.version)
    }
}
