// Copyright 2021-2022 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::Result;
use assorted_debian_utils::excuses::{self, ExcusesItem, Verdict};
use indicatif::{ProgressBar, ProgressIterator};
use log::{debug, error, trace};

use crate::{
    config::{self, CacheEntries, CacheState},
    BaseOptions,
};

pub(crate) struct ProcessUnblocks {
    cache: config::Cache,
}

impl ProcessUnblocks {
    pub(crate) fn new(base_options: BaseOptions) -> Result<Self> {
        Ok(Self {
            cache: config::Cache::new(base_options.force_download, &base_options.mirror)?,
        })
    }

    async fn download_to_cache(&self) -> Result<CacheState> {
        self.cache.download(&[CacheEntries::Excuses]).await
    }

    fn build_unblock(item: &ExcusesItem) -> Option<String> {
        if !Self::is_actionable(item) {
            debug!("{}: not actionable", item.source);
            return None;
        }

        let mut unblock = String::from("unblock ");
        unblock.push_str(&item.source);
        // append _tpu if item is from _tpu
        if item.is_from_tpu() {
            unblock.push_str("_tpu");
        }
        // append version
        unblock.push('/');
        match item.new_version {
            Some(ref version) => unblock.push_str(&version.to_string()),
            _ => {
                // this will never happen
                error!("{}: new-version not set", item.source);
                return None;
            }
        };

        // append architecture for binNMUs
        if item.is_binnmu() {
            unblock.push('/');
            match item.binnmu_arch() {
                Some(arch) => unblock.push_str(&arch.to_string()),
                None => {
                    // this will never happen
                    error!("{}: binNMU but unable to extract architecture", item.source);
                    return None;
                }
            };
        }

        Some(unblock)
    }

    fn is_actionable(item: &ExcusesItem) -> bool {
        if item.is_removal() {
            // skip removals
            trace!("{} not actionable: removal", item.source);
            return false;
        }
        if item.is_from_pu() {
            // skip pu
            trace! {"{} not actionable: pu request", item.source};
            return false;
        }
        if !item.is_from_tpu() && !item.is_binnmu() {
            // skip non-tpu requests
            trace!("{} not actionable: not in tpu or not binnmu", item.source);
            return false;
        }
        if let Some(true) = item.invalidated_by_other_package {
            // skip otherwise blocked packages
            trace!("{} not actionable: invalided by other package", item.source);
            return false;
        }
        if item.migration_policy_verdict != Verdict::RejectedNeedsApproval {
            // skip packages not requiring approval
            trace!("{}: not actionable: does not need approval", item.source);
            return false;
        }

        true
    }

    pub(crate) async fn run(self) -> Result<()> {
        // download excuses and Package files
        self.download_to_cache().await?;
        // parse excuses
        let excuses = excuses::from_reader(self.cache.get_cache_bufreader("excuses.yaml")?)?;

        // now process the excuses
        let pb = ProgressBar::new(excuses.sources.len() as u64);
        pb.set_style(config::default_progress_style().template(
            "{msg}: {spinner:.green} [{wide_bar:.cyan/blue}] {pos}/{len} ({per_sec}, {eta})",
        )?);
        pb.set_message("Processing excuses");
        let unblocks: Vec<_> = excuses
            .sources
            .iter()
            .progress_with(pb)
            .filter_map(Self::build_unblock)
            .collect();

        println!("# Unblocks");
        for unblock in unblocks {
            println!("{}", unblock);
        }
        Ok(())
    }
}
