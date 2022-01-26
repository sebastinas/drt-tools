// Copyright 2021-2022 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashSet;
use std::path::Path;

use anyhow::Result;
use indicatif::{ProgressBar, ProgressIterator};
use serde::Deserialize;

use crate::config;

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
enum MultiArch {
    Allowed,
    Foreign,
    No,
    Same,
}

#[derive(Deserialize, Debug, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
struct BinaryPackage {
    source: Option<String>,
    package: String,
    #[serde(rename = "Multi-Arch")]
    multi_arch: Option<MultiArch>,
}

pub struct SourcePackages {
    ma_same_sources: HashSet<String>,
}

impl SourcePackages {
    pub fn new<P>(paths: &[P]) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let mut ma_same_sources = HashSet::<String>::new();
        for path in paths {
            let sources = Self::parse_packages(path);
            ma_same_sources.extend(sources?);
        }

        Ok(Self { ma_same_sources })
    }

    fn parse_packages<P>(path: P) -> Result<HashSet<String>>
    where
        P: AsRef<Path>,
    {
        // read Package file
        let binary_packages: Vec<BinaryPackage> = rfc822_like::from_file(path.as_ref())?;
        let pb = ProgressBar::new(binary_packages.len() as u64);
        pb.set_style(config::default_progress_style().template(
            "{msg}: {spinner:.green} [{wide_bar:.cyan/blue}] {pos}/{len} ({per_sec}, {eta})",
        ));
        pb.set_message(format!(
            "Processing {}",
            path.as_ref().file_name().unwrap().to_str().unwrap()
        ));
        // collect all sources with MA: same binaries
        let ma_same_sources: HashSet<String> = binary_packages
            .into_iter()
            .progress_with(pb)
            .filter(|binary_package| binary_package.multi_arch == Some(MultiArch::Same))
            .map(|binary_package| {
                if let Some(source_package) = &binary_package.source {
                    source_package.split_whitespace().next().unwrap().into()
                } else {
                    // no Source set, so Source == Package
                    binary_package.package
                }
            })
            .collect();

        Ok(ma_same_sources)
    }

    pub fn is_ma_same(&self, source: &str) -> bool {
        self.ma_same_sources.contains(source)
    }
}
