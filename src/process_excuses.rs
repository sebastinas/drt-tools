// Copyright 2021 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::cmp::min;
use std::collections::HashSet;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use anyhow::Result;
use indicatif::{ProgressBar, ProgressIterator};
use serde::Deserialize;
use structopt::StructOpt;
use xdg::BaseDirectories;

use crate::{config, downloader::*, BaseOptions};
use assorted_debian_utils::wb::WBCommand;
use assorted_debian_utils::{
    architectures::{Architecture, RELEASE_ARCHITECTURES},
    excuses::{self, Component, ExcusesItem, PolicyInfo, Verdict},
    wb::{BinNMU, SourceSpecifier, WBArchitecture, WBCommandBuilder},
};

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

struct SourcePackages {
    ma_same_sources: HashSet<String>,
}

impl SourcePackages {
    fn new<P>(paths: &[P]) -> Result<Self>
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

    fn is_ma_same(&self, source: &str) -> bool {
        self.ma_same_sources.contains(source)
    }
}

#[derive(Debug, StructOpt)]
pub(crate) struct ProcessExcusesOptions {
    /// Do not prepare binNMUs to allow testing migration
    #[structopt(long)]
    no_rebuilds: bool,
}

pub(crate) struct ProcessExcuses {
    base_directory: BaseDirectories,
    base_options: BaseOptions,
    options: ProcessExcusesOptions,
}

impl ProcessExcuses {
    pub(crate) fn new(base_options: BaseOptions, options: ProcessExcusesOptions) -> Result<Self> {
        Ok(Self {
            base_directory: BaseDirectories::with_prefix("Debian-RT-tools")?,
            base_options,
            options,
        })
    }

    async fn download_to_cache(&self) -> Result<CacheState> {
        let downloader = Downloader::new(self.base_options.force_download);

        let urls = [(
            "https://release.debian.org/britney/excuses.yaml",
            "excuses.yaml",
        )];
        for (url, dst) in urls {
            if downloader
                .download_file(url, self.get_cache_path(dst)?)
                .await?
                == CacheState::NoUpdate
                && !self.base_options.force_processing
            {
                // if excuses.yaml did not change, there is nothing new to build
                return Ok(CacheState::NoUpdate);
            }
        }
        for architecture in RELEASE_ARCHITECTURES {
            let url = format!(
                "https://deb.debian.org/debian/dists/unstable/main/binary-{}/Packages.xz",
                architecture
            );
            let dest = format!("Packages_{}", architecture);
            downloader
                .download_file(&url, self.get_cache_path(&dest)?)
                .await?;
        }

        Ok(CacheState::FreshFiles)
    }

    fn get_cache_path<P>(&self, path: P) -> Result<PathBuf>
    where
        P: AsRef<Path>,
    {
        Ok(self.base_directory.place_cache_file(path)?)
    }

    fn is_binnmu_required(policy_info: &PolicyInfo) -> bool {
        if let Some(b) = &policy_info.builtonbuildd {
            if b.verdict == Verdict::Pass {
                // nothing to do
                return false;
            }
            if b.verdict == Verdict::RejectedCannotDetermineIfPermanent {
                // missing builds
                return false;
            }
        }
        if let Some(a) = &policy_info.age {
            if a.current_age < min(a.age_requirement / 2, a.age_requirement - 1) {
                // too young
                return false;
            }
        }

        // if the others do not pass, would not migrate even if binNMUed
        policy_info
            .extras
            .values()
            .all(|info| info.verdict == Verdict::Pass)
    }

    fn build_binnmu(item: &ExcusesItem, source_packages: &SourcePackages) -> Option<WBCommand> {
        if !Self::is_actionable(item) {
            return None;
        }

        if let Some(policy_info) = &item.policy_info {
            if !Self::is_binnmu_required(policy_info) {
                return None;
            }

            // find architectures with maintainer built binaries
            let mut archs = vec![];
            for (arch, signer) in policy_info.builtonbuildd.as_ref().unwrap().signed_by.iter() {
                if let Some(signer) = signer {
                    if !signer.ends_with("@buildd.debian.org") {
                        archs.push(WBArchitecture::Architecture(arch.clone()));
                    }
                }
            }
            if archs.is_empty() {
                // this should not happen, but just to be on the safe side
                return None;
            }
            if archs.contains(&WBArchitecture::Architecture(Architecture::All)) {
                // cannot binNMU arch:all
                return None;
            }

            let mut source_specifier = SourceSpecifier::new(&item.source);
            source_specifier.with_version(&item.new_version);
            if !source_packages.is_ma_same(&item.source) {
                source_specifier.with_architectures(&archs);
            }
            Some(BinNMU::new(&source_specifier, "Rebuild on buildd").build())
        } else {
            None
        }
    }

    fn is_actionable(item: &ExcusesItem) -> bool {
        if item.new_version == "-" {
            // skip removals
            return false;
        }
        if item.new_version == item.old_version {
            // skip binNMUs
            return false;
        }
        if item.item_name.ends_with("_pu") {
            // skip PU requests
            return false;
        }
        match item.component {
            Some(Component::Main) | None => {}
            _ => {
                // skip non-free and contrib
                return false;
            }
        }
        if let Some(true) = item.invalidated_by_other_package {
            // skip otherwise blocked packages
            return false;
        }
        if item.missing_builds.is_some() {
            // skip packages with missing builds
            return false;
        }

        true
    }

    #[tokio::main]
    pub(crate) async fn run(self) -> Result<()> {
        // download excuses and Package files
        if self.download_to_cache().await? == CacheState::NoUpdate {
            // nothing to do
            return Ok(());
        }

        let mut all_paths = vec![];
        for architecture in RELEASE_ARCHITECTURES {
            all_paths.push(self.get_cache_path(format!("Packages_{}", architecture))?);
        }
        let source_packages = SourcePackages::new(&all_paths)?;

        // parse excuses
        let excuses = excuses::from_reader(BufReader::new(File::open(
            self.get_cache_path("excuses.yaml")?,
        )?))?;

        // now process the excuses
        let pb = ProgressBar::new(excuses.sources.len() as u64);
        pb.set_style(config::default_progress_style().template(
            "{msg}: {spinner:.green} [{wide_bar:.cyan/blue}] {pos}/{len} ({per_sec}, {eta})",
        ));
        pb.set_message("Processing excuses");
        let to_binnmu: Vec<WBCommand> = excuses
            .sources
            .iter()
            .progress_with(pb)
            .filter_map(|item| Self::build_binnmu(item, &source_packages))
            .collect();

        if !self.options.no_rebuilds {
            println!("# Rebuild on buildds for testing migration");
            for binnmu in to_binnmu {
                println!("{}", binnmu);
                if !self.base_options.dry_run {
                    binnmu.execute()?;
                }
            }
        }
        Ok(())
    }
}
