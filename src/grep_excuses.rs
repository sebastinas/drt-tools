// Copyright 2022-2023 Sebastian Ramacher
// SPDX-License-Identifier: LGPL-3.0-or-later

use anyhow::Result;
use assorted_debian_utils::{
    autoremovals::{self, AutoRemoval},
    excuses::{self, ExcusesItem},
};
use async_trait::async_trait;
use chrono::Utc;
use clap::Parser;

use crate::{
    config::{self, CacheEntries},
    Command,
};

#[derive(Debug, Parser)]
pub(crate) struct GrepExcusesOptions {
    /// Currently not implemented
    #[clap(long)]
    autopkgtests: bool,
    /// The maintainer or package to grep for
    maintainer_package: String,
}

pub(crate) struct GrepExcuses<'a> {
    cache: &'a config::Cache,
    options: GrepExcusesOptions,
}

impl<'a> GrepExcuses<'a> {
    pub(crate) fn new(cache: &'a config::Cache, options: GrepExcusesOptions) -> Self {
        Self { cache, options }
    }

    fn print_excuse(&self, excuse: &ExcusesItem) {
        println!(
            "{} ({} to {})",
            excuse.source,
            excuse
                .old_version
                .as_ref()
                .map_or_else(|| "-".into(), |version| version.to_string()),
            excuse
                .new_version
                .as_ref()
                .map_or_else(|| "-".into(), |version| version.to_string())
        );
        if let Some(maintainer) = &excuse.maintainer {
            println!("  Maintainer: {}", maintainer);
        }
        for line in &excuse.excuses {
            println!("  {}", voca_rs::strip::strip_tags(line));
        }
    }

    fn print_autoremoal(&self, autoremoval: &AutoRemoval) {
        println!("{} (AUTOREMOVAL)", autoremoval.source);
        let time_diff = autoremoval.removal_date - Utc::now();
        println!(
            "  flagged for removal in {} days ({})",
            time_diff.num_days(),
            autoremoval.removal_date.to_rfc2822()
        );
        // TODO: print other fields
    }
}

#[async_trait]
impl Command for GrepExcuses<'_> {
    async fn run(&self) -> Result<()> {
        // parse excuses
        let excuses = excuses::from_reader(self.cache.get_cache_bufreader("excuses.yaml")?)?;
        // parse autoremovals
        let autoremovals =
            autoremovals::from_reader(self.cache.get_cache_bufreader("autoremovals.yaml")?)?;

        // first print the autoremoval
        if let Some(autoremoval) = autoremovals.get(&self.options.maintainer_package) {
            self.print_autoremoal(autoremoval);
        }

        // then print the excuses
        for excuse in excuses.sources {
            if excuse.source == self.options.maintainer_package {
                self.print_excuse(&excuse);
                continue;
            }
            if let Some(maintainer) = &excuse.maintainer {
                if maintainer == &self.options.maintainer_package {
                    self.print_excuse(&excuse);
                    continue;
                }
            }
        }

        Ok(())
    }

    fn downloads(&self) -> Vec<CacheEntries> {
        [CacheEntries::Excuses, CacheEntries::AutoRemovals].into()
    }
}
