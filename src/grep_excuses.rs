// Copyright 2022 Sebastian Ramacher
// SPDX-License-Identifier: LGPL-3.0-or-later

use anyhow::Result;
use assorted_debian_utils::{
    autoremovals::{self, AutoRemoval},
    excuses::{self, ExcusesItem},
};
use chrono::Utc;
use clap::Parser;

use crate::{
    config::{self, CacheEntries, CacheState},
    BaseOptions,
};

#[derive(Debug, Parser)]
pub(crate) struct GrepExcusesOptions {
    /// Currently not implemented
    #[clap(long)]
    autopkgtests: bool,
    /// The maintainer or package to grep for
    maintainer_package: String,
}

pub(crate) struct GrepExcuses {
    cache: config::Cache,
    options: GrepExcusesOptions,
}

impl GrepExcuses {
    pub(crate) fn new(base_options: BaseOptions, options: GrepExcusesOptions) -> Result<Self> {
        Ok(Self {
            cache: config::Cache::new(base_options.force_download)?,
            options,
        })
    }

    async fn download_to_cache(&self) -> Result<CacheState> {
        self.cache
            .download(&[CacheEntries::Excuses, CacheEntries::AutoRemovals])
            .await
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

    pub(crate) async fn run(self) -> Result<()> {
        self.download_to_cache().await?;

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
}
