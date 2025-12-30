// Copyright 2021-2024 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{cmp::min, collections::HashSet};

use anyhow::Result;
use assorted_debian_utils::{
    architectures::Architecture,
    archive::{Component, SuiteOrCodename},
    excuses::{self, ExcusesItem, PolicyInfo, Verdict},
    wb::{BinNMU, SourceSpecifier, WBArchitecture, WBCommand, WBCommandBuilder},
};
use async_trait::async_trait;
use indicatif::{ProgressBar, ProgressIterator};
use log::{debug, error, info, trace, warn};
use serde::{Deserialize, Serialize};

use crate::{
    AsyncCommand, Downloads,
    cli::{BaseOptions, ProcessExcusesOptions},
    config::{self, CacheEntries, CachePaths, default_progress_template},
    source_packages::SourcePackages,
    utils::execute_wb_commands,
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
        self.binnmus.push(command.clone());
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
enum Action {
    BinNMU(WBCommand),
    Unblock(String),
}

pub(crate) struct ProcessExcuses<'a> {
    cache: &'a config::Cache,
    base_options: &'a BaseOptions,
    options: ProcessExcusesOptions,
}

impl<'a> ProcessExcuses<'a> {
    pub(crate) fn new(
        cache: &'a config::Cache,
        base_options: &'a BaseOptions,
        options: ProcessExcusesOptions,
    ) -> Self {
        Self {
            cache,
            base_options,
            options,
        }
    }

    fn is_binnmu_required(&self, policy_info: &PolicyInfo) -> bool {
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
        if let Some(a) = &policy_info.age
            && !self.options.ignore_age
            && a.current_age < min(a.age_requirement / 2, a.age_requirement - 1)
        {
            // too young
            trace!(
                "no binnmu possible: too young: {} days (required: {} days)",
                a.current_age, a.age_requirement
            );
            return false;
        }
        if let Some(autopkgtests) = &policy_info.autopkgtest
            && !self.options.ignore_autopkgtests
            && autopkgtests.verdict != Verdict::Pass
        {
            trace!("no binnmu possible: autopkgtests are not passing/are pending");
            return false;
        }

        // if the others do not pass, would not migrate even if binNMUed
        policy_info.extras.values().all(|info| {
            if info.verdict == Verdict::Pass {
                true
            } else {
                trace!("no binnmu possible: verdict not passing: {info:?}");
                false
            }
        })
    }

    fn build_binnmu(
        &self,
        item: &ExcusesItem,
        source_packages: &SourcePackages,
    ) -> Option<WBCommand> {
        let policy_info = item.policy_info.as_ref()?;
        if !self.is_binnmu_required(policy_info) {
            debug!("{}: binNMU not required", item.source);
            return None;
        }

        // find architectures with maintainer built binaries
        let mut archs = vec![];
        for (arch, signer) in &policy_info.builtonbuildd.as_ref().unwrap().signed_by {
            if let Some(signer) = signer
                && !signer.ends_with("@buildd.debian.org")
            {
                if arch == &Architecture::All {
                    // cannot binNMU arch: all
                    debug!("{}: cannot binNMU arch: all", item.source);
                    return None;
                }
                archs.push(WBArchitecture::Architecture(*arch));
            }
        }
        if archs.is_empty() {
            // this should not happen, but just to be on the safe side
            warn!(
                "{}: considered candidate, but no architecture with missing build",
                item.source
            );
            trace!("{item:?}");
            return None;
        }

        let mut source_specifier = SourceSpecifier::new(&item.source);
        source_specifier.with_version(item.new_version.as_ref().unwrap());
        if !source_packages.is_ma_same(&item.source) {
            source_specifier.with_architectures(&archs);
        }
        if let Ok(command) = BinNMU::new(&source_specifier, "Rebuild on buildd") {
            Some(command.build())
        } else {
            error!("{}: failed to construct nmu command", item.source);
            None
        }
    }

    fn build_action(&self, item: &ExcusesItem, source_packages: &SourcePackages) -> Option<Action> {
        if !Self::is_actionable(item) {
            debug!("{}: not actionable", item.source);
            return None;
        }

        if Self::is_unblock_actionable(item) {
            Self::build_unblock(item).map(Action::Unblock)
        } else if Self::is_binnmu_actionable(item) {
            self.build_binnmu(item, source_packages).map(Action::BinNMU)
        } else {
            None
        }
    }

    fn build_unblock(item: &ExcusesItem) -> Option<String> {
        let mut unblock = String::from("unblock ");
        unblock.push_str(item.source.as_ref());
        // append _tpu if item is from _tpu
        if item.is_from_tpu() {
            unblock.push_str("_tpu");
        }
        // append version
        unblock.push('/');
        if let Some(ref version) = item.new_version {
            unblock.push_str(&version.to_string());
        } else {
            // this will never happen
            error!("{}: new-version not set", item.source);
            return None;
        };

        // append architecture for binNMUs
        if item.is_binnmu() {
            unblock.push('/');
            if let Some(arch) = item.binnmu_arch() {
                unblock.push_str(arch.as_ref());
            } else {
                // this will never happen
                error!("{}: binNMU but unable to extract architecture", item.source);
                return None;
            };
        }

        Some(unblock)
    }

    fn is_actionable(item: &ExcusesItem) -> bool {
        if item.is_removal() {
            // skip removals
            info!("{} not actionable: removal", item.source);
            return false;
        }
        if item.is_from_pu() {
            // skip PU requests
            info!("{} not actionable: pu request", item.source);
            return false;
        }
        if let Some(true) = item.invalidated_by_other_package {
            // skip otherwise blocked packages
            info!("{} not actionable: invalided by other package", item.source);
            return false;
        }

        true
    }

    fn is_binnmu_actionable(item: &ExcusesItem) -> bool {
        if item.is_from_tpu() {
            // skip TPU requests
            info!("{} not actionable: tpu request", item.source);
            return false;
        }
        match item.component {
            Some(Component::Main) | None => {}
            Some(component) => {
                // skip non-free and contrib
                info!("{} not actionable: in {}", item.source, component);
                return false;
            }
        }
        if item.missing_builds.is_some() {
            // skip packages with missing builds
            info!("{} not actionable: missing builds", item.source);
            return false;
        }

        true
    }

    fn is_unblock_actionable(item: &ExcusesItem) -> bool {
        if !item.is_from_tpu() && !item.is_binnmu() {
            // skip non-tpu requests
            trace!("{} not actionable: not in tpu or not binnmu", item.source);
            return false;
        }
        if item.migration_policy_verdict != Verdict::RejectedNeedsApproval {
            // skip packages not requiring approval
            trace!("{}: not actionable: does not need approval", item.source);
            return false;
        }

        true
    }

    fn load_scheduled_binnmus(&self) -> ScheduledBinNMUs {
        if let Ok(reader) = self.cache.get_data_bufreader(SCHEDULED_BINNMUS) {
            serde_yaml::from_reader(reader).unwrap_or_default()
        } else {
            ScheduledBinNMUs::default()
        }
    }

    fn store_scheduled_binnmus(&self, scheduled_binnmus: &ScheduledBinNMUs) -> Result<()> {
        serde_yaml::to_writer(
            self.cache.get_data_bufwriter(SCHEDULED_BINNMUS)?,
            scheduled_binnmus,
        )
        .map_err(Into::into)
    }
}

#[async_trait]
impl AsyncCommand for ProcessExcuses<'_> {
    async fn run(&self) -> Result<()> {
        let source_packages = SourcePackages::new(
            &self
                .cache
                .get_package_paths(SuiteOrCodename::UNSTABLE, false)?,
        )?;
        // parse excuses
        let excuses = excuses::from_reader(self.cache.get_cache_bufreader("excuses.yaml")?)?;

        // load already scheduled binNMUs from cache
        let mut scheduled_binnmus = self.load_scheduled_binnmus();

        // now process the excuses
        let pb = ProgressBar::new(excuses.sources.len() as u64);
        pb.set_style(config::default_progress_style().template(default_progress_template())?);
        pb.set_message("Processing excuses");
        let actions: HashSet<Action> = excuses
            .sources
            .iter()
            .progress_with(pb)
            .filter_map(|item| self.build_action(item, &source_packages))
            .filter(|action| match action {
                Action::BinNMU(command) => {
                    if scheduled_binnmus.contains(command) {
                        info!("{command}: skipping, already scheduled");
                        false
                    } else {
                        if !self.base_options.dry_run {
                            scheduled_binnmus.store(command);
                        }
                        true
                    }
                }
                Action::Unblock(_) => true,
            })
            .collect();

        println!("# Unblocks");
        let binnmus: Vec<_> = actions
            .into_iter()
            .filter_map(|action| match action {
                Action::BinNMU(command) => Some(command),
                Action::Unblock(unblock) => {
                    println!("{unblock}");
                    None
                }
            })
            .collect();

        println!("# Rebuild on buildds for testing migration");
        execute_wb_commands(binnmus, self.base_options).await?;

        // store scheduled binNMUs in cache
        self.store_scheduled_binnmus(&scheduled_binnmus)
    }
}

impl Downloads for ProcessExcuses<'_> {
    fn required_downloads(&self) -> Vec<CacheEntries> {
        vec![CacheEntries::Excuses]
    }

    fn downloads(&self) -> Vec<CacheEntries> {
        vec![CacheEntries::Packages(SuiteOrCodename::UNSTABLE)]
    }
}
