// Copyright 2022-2024 Sebastian Ramacher
// SPDX-License-Identifier: LGPL-3.0-or-later

use std::fmt::Display;

use anyhow::Result;
use assorted_debian_utils::{
    autoremovals::{self, AutoRemoval},
    excuses::{self, ExcusesItem},
    version::PackageVersion,
};
use chrono::Utc;
use clap::Parser;

use crate::{
    config::{self, CacheEntries},
    Command, Downloads,
};

#[derive(Debug, Parser)]
pub(crate) struct GrepExcusesOptions {
    /// Currently not implemented
    #[clap(long)]
    autopkgtests: bool,
    /// The maintainer or package to grep for
    #[clap(num_args = 1, required = true)]
    maintainer_package: Vec<String>,
}

pub(crate) struct GrepExcuses<'a> {
    cache: &'a config::Cache,
    options: GrepExcusesOptions,
}

impl<'a> GrepExcuses<'a> {
    pub(crate) fn new(cache: &'a config::Cache, options: GrepExcusesOptions) -> Self {
        Self { cache, options }
    }
}

fn print_excuse(excuse: &ExcusesItem) {
    println!(
        "{} ({} to {})",
        excuse.source,
        excuse
            .old_version
            .as_ref()
            .map_or_else(|| "-".into(), PackageVersion::to_string),
        excuse
            .new_version
            .as_ref()
            .map_or_else(|| "-".into(), PackageVersion::to_string)
    );
    if let Some(maintainer) = &excuse.maintainer {
        println!("  Maintainer: {maintainer}");
    }
    for line in &excuse.excuses {
        println!("  {}", voca_rs::strip::strip_tags(line));
    }
}

fn print_autoremoval(autoremoval: &AutoRemoval) {
    fn print_indented<T>(items: &[T])
    where
        T: Display,
    {
        for item in items {
            println!("    - {item}");
        }
    }

    println!("{} (AUTOREMOVAL)", autoremoval.source);
    let time_diff = autoremoval.removal_date - Utc::now();
    println!(
        "  flagged for removal in {} days ({})",
        time_diff.num_days(),
        autoremoval.removal_date.to_rfc2822()
    );
    println!("    bugs:");
    print_indented(&autoremoval.bugs);
    println!("    dependencies only: {}", autoremoval.dependencies_only);
    if let Some(ref rdeps) = autoremoval.rdeps {
        println!("    reverse dependencies:");
        print_indented(rdeps);
    }
    if let Some(ref dependencies) = autoremoval.buggy_dependencies {
        println!("    buggy dependencies:");
        print_indented(dependencies);
    }
    if let Some(ref bugs_dependencies) = autoremoval.bugs_dependencies {
        println!("    bugs in dependencies:");
        print_indented(bugs_dependencies);
    }
    println!("    version: {}", autoremoval.version);
    println!(
        "    last checked: {}",
        autoremoval.last_checked.to_rfc2822()
    );
}

impl Command for GrepExcuses<'_> {
    fn run(&self) -> Result<()> {
        // parse excuses
        let excuses = excuses::from_reader(self.cache.get_cache_bufreader("excuses.yaml")?)?;
        // parse autoremovals
        let autoremovals =
            autoremovals::from_reader(self.cache.get_cache_bufreader("autoremovals.yaml")?)?;

        for maintainer_package in &self.options.maintainer_package {
            // first print the autoremoval
            if let Some(autoremoval) = autoremovals.get(maintainer_package) {
                print_autoremoval(autoremoval);
            }

            // then print the excuses
            for excuse in &excuses.sources {
                if excuse.source == *maintainer_package {
                    print_excuse(excuse);
                    continue;
                }
                if let Some(maintainer) = &excuse.maintainer {
                    if maintainer == maintainer_package {
                        print_excuse(excuse);
                        continue;
                    }
                }
            }
        }

        Ok(())
    }
}

impl Downloads for GrepExcuses<'_> {
    fn downloads(&self) -> Vec<CacheEntries> {
        vec![CacheEntries::Excuses, CacheEntries::AutoRemovals]
    }
}
