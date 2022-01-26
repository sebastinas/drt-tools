// Copyright 2021 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::cmp::min;

use anyhow::Result;
use indicatif::{ProgressBar, ProgressIterator};
use serde::{Deserialize, Serialize};
use structopt::StructOpt;

use crate::{config, downloader::*, source_packages::SourcePackages, BaseOptions};
use assorted_debian_utils::{
    architectures::{Architecture, RELEASE_ARCHITECTURES},
    excuses::{self, Component, ExcusesItem, PolicyInfo, Verdict},
    wb::{BinNMU, SourceSpecifier, WBArchitecture, WBCommand, WBCommandBuilder},
};

#[derive(Debug, Default, Serialize, Deserialize)]
struct ScheduledBinNMUs {
    binnmus: Vec<WBCommand>,
}

impl ScheduledBinNMUs {
    fn contains(&self, command: &WBCommand) -> bool {
        self.binnmus.contains(command)
    }

    fn store(&mut self, command: &WBCommand) {
        self.binnmus.push(command.clone())
    }
}

#[derive(Debug, StructOpt)]
pub(crate) struct ProcessExcusesOptions {
    /// Do not prepare binNMUs to allow testing migration
    #[structopt(long)]
    no_rebuilds: bool,
}

pub(crate) struct ProcessExcuses {
    cache: config::Cache,
    base_options: BaseOptions,
    options: ProcessExcusesOptions,
}

impl ProcessExcuses {
    pub(crate) fn new(base_options: BaseOptions, options: ProcessExcusesOptions) -> Result<Self> {
        Ok(Self {
            cache: config::Cache::new()?,
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
                .download_file(url, self.cache.get_cache_path(dst)?)
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
                .download_file(&url, self.cache.get_cache_path(&dest)?)
                .await?;
        }

        Ok(CacheState::FreshFiles)
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

    fn load_scheduled_binnmus(&self) -> ScheduledBinNMUs {
        if let Ok(reader) = self.cache.get_data_bufreader("scheduled-binnmus.yaml") {
            serde_yaml::from_reader(reader).unwrap_or_default()
        } else {
            Default::default()
        }
    }

    fn store_scheduled_binnmus(&self, scheduled_binnmus: ScheduledBinNMUs) -> Result<()> {
        serde_yaml::to_writer(
            self.cache.get_data_bufwriter("scheduled-binnmus.yaml")?,
            &scheduled_binnmus,
        )?;
        Ok(())
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
            all_paths.push(
                self.cache
                    .get_cache_path(format!("Packages_{}", architecture))?,
            );
        }
        let source_packages = SourcePackages::new(&all_paths)?;

        // parse excuses
        let excuses = excuses::from_reader(self.cache.get_cache_bufreader("excuses.yaml")?)?;

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

        // load already scheduled binNMUs from cache
        let mut scheduled_binnmus = self.load_scheduled_binnmus();

        if !self.options.no_rebuilds {
            println!("# Rebuild on buildds for testing migration");
            for binnmu in to_binnmu {
                if scheduled_binnmus.contains(&binnmu) {
                    println!("# already scheduled: {}", binnmu);
                } else {
                    println!("{}", binnmu);
                    if !self.base_options.dry_run {
                        binnmu.execute()?;
                        scheduled_binnmus.store(&binnmu);
                    }
                }
            }
        }

        // store scheduled binNMUs in cache
        self.store_scheduled_binnmus(scheduled_binnmus)
    }
}
