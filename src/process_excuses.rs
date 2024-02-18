// Copyright 2021-2023 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::cmp::min;

use anyhow::Result;
use assorted_debian_utils::{
    architectures::Architecture,
    archive::{Component, Suite},
    excuses::{self, ExcusesItem, PolicyInfo, Verdict},
    wb::{BinNMU, SourceSpecifier, WBArchitecture, WBCommand, WBCommandBuilder},
};
use async_trait::async_trait;
use indicatif::{ProgressBar, ProgressIterator};
use log::{debug, error, info, trace, warn};
use serde::{Deserialize, Serialize};

use crate::{
    config::{self, default_progress_template, CacheEntries},
    source_packages::SourcePackages,
    utils::execute_wb_commands,
    AsyncCommand, BaseOptions, Downloads,
};

const SCHEDULED_BINNMUS: &str = "scheduled-binnmus.yaml";

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

pub(crate) struct ProcessExcuses<'a> {
    cache: &'a config::Cache,
    base_options: &'a BaseOptions,
}

impl<'a> ProcessExcuses<'a> {
    pub(crate) fn new(cache: &'a config::Cache, base_options: &'a BaseOptions) -> Self {
        Self {
            cache,
            base_options,
        }
    }

    fn is_binnmu_required(policy_info: &PolicyInfo) -> bool {
        if let Some(b) = &policy_info.builtonbuildd {
            if b.verdict == Verdict::Pass {
                // nothing to do
                trace!("no binmu required: passing");
                return false;
            }
            if b.verdict == Verdict::RejectedCannotDetermineIfPermanent {
                // missing builds
                trace!("no binnmu possible: missing builds");
                return false;
            }
        }
        if let Some(a) = &policy_info.age {
            if a.current_age < min(a.age_requirement / 2, a.age_requirement - 1) {
                // too young
                trace!(
                    "no binnmu possible: too young: {} days (required: {} days)",
                    a.current_age,
                    a.age_requirement
                );
                return false;
            }
        }

        // if the others do not pass, would not migrate even if binNMUed
        policy_info.extras.values().all(|info| {
            if info.verdict != Verdict::Pass {
                trace!("no binnmu possible: verdict not passing: {:?}", info);
                false
            } else {
                true
            }
        })
    }

    fn build_binnmu(item: &ExcusesItem, source_packages: &SourcePackages) -> Option<WBCommand> {
        if !Self::is_actionable(item) {
            debug!("{}: not actionable", item.source);
            return None;
        }

        let Some(ref policy_info) = item.policy_info else {
            return None;
        };
        if !Self::is_binnmu_required(policy_info) {
            debug!("{}: binNMU not required", item.source);
            return None;
        }

        // find architectures with maintainer built binaries
        let mut archs = vec![];
        for (arch, signer) in policy_info.builtonbuildd.as_ref().unwrap().signed_by.iter() {
            if let Some(signer) = signer {
                if !signer.ends_with("@buildd.debian.org") {
                    if arch == &Architecture::All {
                        // cannot binNMU arch: all
                        debug!("{}: cannot binNMU arch: all", item.source);
                        return None;
                    }
                    archs.push(WBArchitecture::Architecture(*arch));
                }
            }
        }
        if archs.is_empty() {
            // this should not happen, but just to be on the safe side
            warn!(
                "{}: considered candidate, but no architecture with missing build",
                item.source
            );
            trace!("{:?}", item);
            return None;
        }

        let mut source_specifier = SourceSpecifier::new(&item.source);
        source_specifier.with_version(item.new_version.as_ref().unwrap());
        if !source_packages.is_ma_same(&item.source) {
            source_specifier.with_architectures(&archs);
        }
        match BinNMU::new(&source_specifier, "Rebuild on buildd") {
            Ok(command) => Some(command.build()),
            // not binNMU-able
            Err(_) => {
                error!("{}: failed to construct nmu command", item.source);
                None
            }
        }
    }

    fn is_actionable(item: &ExcusesItem) -> bool {
        if item.is_removal() {
            // skip removals
            trace!("{} not actionable: removal", item.source);
            return false;
        }
        if item.is_binnmu() {
            // skip binNMUs
            trace!("{} not actionable: binNMU", item.source);
            return false;
        }
        if item.is_from_pu() {
            // skip PU requests
            trace!("{} not actionable: pu request", item.source);
            return false;
        }
        if item.is_from_tpu() {
            // skip TPU requests
            trace!("{} not actionable: tpu request", item.source);
            return false;
        }
        match item.component {
            Some(Component::Main) | None => {}
            _ => {
                // skip non-free and contrib
                trace!("{} not actionable: in {:?}", item.source, item.component);
                return false;
            }
        }
        if let Some(true) = item.invalidated_by_other_package {
            // skip otherwise blocked packages
            trace!("{} not actionable: invalided by other package", item.source);
            return false;
        }
        if item.missing_builds.is_some() {
            // skip packages with missing builds
            trace!("{} not actionable: missing builds", item.source);
            return false;
        }

        true
    }

    fn load_scheduled_binnmus(&self) -> ScheduledBinNMUs {
        if let Ok(reader) = self.cache.get_data_bufreader(SCHEDULED_BINNMUS) {
            serde_yaml::from_reader(reader).unwrap_or_default()
        } else {
            Default::default()
        }
    }

    fn store_scheduled_binnmus(&self, scheduled_binnmus: ScheduledBinNMUs) -> Result<()> {
        serde_yaml::to_writer(
            self.cache.get_data_bufwriter(SCHEDULED_BINNMUS)?,
            &scheduled_binnmus,
        )
        .map_err(|err| err.into())
    }
}

#[async_trait]
impl AsyncCommand for ProcessExcuses<'_> {
    async fn run(&self) -> Result<()> {
        let source_packages =
            SourcePackages::new(&self.cache.get_package_paths(Suite::Unstable, false)?)?;
        // parse excuses
        let excuses = excuses::from_reader(self.cache.get_cache_bufreader("excuses.yaml")?)?;

        // load already scheduled binNMUs from cache
        let mut scheduled_binnmus = self.load_scheduled_binnmus();

        // now process the excuses
        let pb = ProgressBar::new(excuses.sources.len() as u64);
        pb.set_style(config::default_progress_style().template(default_progress_template())?);
        pb.set_message("Processing excuses");
        let to_binnmu: Vec<WBCommand> = excuses
            .sources
            .iter()
            .progress_with(pb)
            .filter_map(|item| Self::build_binnmu(item, &source_packages))
            .filter(|command| {
                if scheduled_binnmus.contains(command) {
                    info!("{}: skipping, already scheduled", command);
                    false
                } else {
                    if !self.base_options.dry_run {
                        scheduled_binnmus.store(command);
                    }
                    true
                }
            })
            .collect();

        println!("# Rebuild on buildds for testing migration");
        execute_wb_commands(to_binnmu, self.base_options.dry_run).await?;

        // store scheduled binNMUs in cache
        self.store_scheduled_binnmus(scheduled_binnmus)
    }
}

impl Downloads for ProcessExcuses<'_> {
    fn required_downloads(&self) -> Vec<CacheEntries> {
        vec![CacheEntries::Excuses]
    }

    fn downloads(&self) -> Vec<CacheEntries> {
        vec![CacheEntries::Packages(Suite::Unstable)]
    }
}
