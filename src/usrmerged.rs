// Copyright 2022 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};

use anyhow::Result;
use assorted_debian_utils::architectures::{Architecture, RELEASE_ARCHITECTURES};
use assorted_debian_utils::archive::Suite;
use clap::Parser;
// use indicatif::{ProgressBar, ProgressIterator};
use log::{debug, info, warn};

use crate::{
    config::{self, CacheEntries},
    BaseOptions,
};

fn strip_section(package: &str) -> String {
    package.split_once('/').map_or(package, |(_, p)| p).into()
}

fn load_contents(
    cache: &config::Cache,
    suite: Suite,
    arch: Architecture,
) -> Result<HashMap<String, HashSet<String>>> {
    let mut file_map: HashMap<String, HashSet<String>> = HashMap::new();

    for (architecture, path) in cache.get_content_paths(suite)? {
        if arch != architecture {
            continue;
        }

        log::debug!(
            "Processing contents for {} on {}: {:?}",
            suite,
            architecture,
            path
        );

        let reader = BufReader::new(File::open(path)?);
        for line in reader.lines() {
            let line = match line {
                Ok(inner_line) => inner_line,
                Err(_) => break,
            };

            let mut split = line.split_whitespace();
            let (path, packages) = match (split.next(), split.next()) {
                (Some(path), Some(packages)) => (path, packages),
                _ => {
                    warn!("Unable to process line: {}", line);
                    continue;
                }
            };

            let packages = packages.split(',');
            match file_map.get_mut(path) {
                Some(entry) => {
                    entry.extend(packages.map(strip_section));
                }
                None => {
                    file_map.insert(path.into(), packages.map(strip_section).collect());
                }
            }
        }
    }
    Ok(file_map)
}

#[derive(Debug, Parser)]
pub(crate) struct UsrMergedOptions {}

pub(crate) struct UsrMerged {
    cache: config::Cache,
}

impl UsrMerged {
    pub(crate) fn new(base_options: BaseOptions, _: UsrMergedOptions) -> Result<Self> {
        Ok(Self {
            cache: config::Cache::new(base_options.force_download)?,
        })
    }

    #[tokio::main]
    async fn download_to_cache(&self) -> Result<()> {
        self.cache
            .download(&[
                CacheEntries::Contents(Suite::Stable(None)),
                CacheEntries::Contents(Suite::Testing(None)),
            ])
            .await?;
        Ok(())
    }

    pub(crate) fn run(self) -> Result<()> {
        self.download_to_cache()?;

        for architecture in RELEASE_ARCHITECTURES
            .into_iter()
            .chain([Architecture::All].into_iter())
        {
            let stable_file_map = load_contents(&self.cache, Suite::Stable(None), architecture)?;
            let testing_file_map = load_contents(&self.cache, Suite::Testing(None), architecture)?;

            /*
            let pb = ProgressBar::new(stable_file_map.len() as u64);
            pb.set_style(config::default_progress_style().template(
                "{msg}: {spinner:.green} [{wide_bar:.cyan/blue}] {pos}/{len} ({per_sec}, {eta})",
            ));
            pb.set_message("Processing contents");
            */

            for (path, stable_packages) in &stable_file_map {
                // .iter().progress_with(pb) {
                let path_to_test = if let Some(stripped) = path.strip_prefix("usr/") {
                    stripped.into()
                } else {
                    format!("usr/{}", path)
                };
                debug!(
                    "{}: processing {} - checking for {}",
                    architecture, path, path_to_test
                );

                let testing_packages = match testing_file_map.get(&path_to_test) {
                    Some(packages) => packages,
                    None => continue,
                };

                if stable_packages == testing_packages {
                    info!(
                        "Renamed {} to {} (packages {:?})",
                        path, path_to_test, testing_packages,
                    );
                } else {
                    println!(
                        "E: {}: Renamed {} to {} and packages changed: {:?} vs {:?}",
                        architecture, path, path_to_test, stable_packages, testing_packages,
                    );
                }
            }
        }

        Ok(())
    }
}
