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
use smallvec::SmallVec;
use smartstring::{LazyCompact, SmartString};

use crate::{
    config::{self, CacheEntries},
    BaseOptions,
};

type SmallString = SmartString<LazyCompact>;
// if there is a file in more than one package, the most common case are two packages
type LoadIterator = dyn Iterator<Item = (SmallString, SmallVec<[SmallString; 2]>)>;

fn strip_section(package: &str) -> SmallString {
    package.split_once('/').map_or(package, |(_, p)| p).into()
}

#[derive(Debug, Parser)]
pub(crate) struct UsrMergedOptions {
    #[clap(long)]
    /// Also include files that only moved between / and /usr
    only_files_moved: bool,
    #[clap(long)]
    /// Do not skip some paths known to not be affected
    no_skip: bool,
}

pub(crate) struct UsrMerged {
    cache: config::Cache,
    options: UsrMergedOptions,
}

impl UsrMerged {
    pub(crate) fn new(base_options: BaseOptions, options: UsrMergedOptions) -> Result<Self> {
        Ok(Self {
            cache: config::Cache::new(base_options.force_download)?,
            options,
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

    fn load_contents_iter(&self, suite: Suite, arch: Architecture) -> Result<Box<LoadIterator>> {
        for (architecture, path) in self.cache.get_content_paths(suite)? {
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
            return Ok(Box::new(reader.lines().filter_map(|line| {
                let line = match line {
                    Ok(line) => line,
                    _ => {
                        return None;
                    }
                };

                let mut split = line.split_whitespace();
                let (path, packages) = match (split.next(), split.next()) {
                    (Some(path), Some(packages)) => (path, packages),
                    _ => {
                        warn!("Unable to process line: {}", line);
                        return None;
                    }
                };

                // there are no packages with files in boot/, usr/etc/, and usr/lib/modules/
                if path.starts_with("boot/")
                    || path.starts_with("etc/")
                    || path.starts_with("lib/modules/")
                {
                    return None;
                }

                Some((
                    path.into(),
                    packages.split(',').map(strip_section).collect(),
                ))
            })));
        }

        unreachable!("This will never be reached.");
    }

    fn load_contents(
        &self,
        suite: Suite,
        arch: Architecture,
    ) -> Result<HashMap<SmallString, SmallVec<[SmallString; 2]>>> {
        Ok(HashMap::from_iter(self.load_contents_iter(suite, arch)?))
    }

    pub(crate) fn run(self) -> Result<()> {
        self.download_to_cache()?;

        for architecture in RELEASE_ARCHITECTURES
            .into_iter()
            .chain([Architecture::All].into_iter())
        {
            let testing_file_map = self.load_contents(Suite::Testing(None), architecture)?;

            /*
            let pb = ProgressBar::new(stable_file_map.len() as u64);
            pb.set_style(config::default_progress_style().template(
                "{msg}: {spinner:.green} [{wide_bar:.cyan/blue}] {pos}/{len} ({per_sec}, {eta})",
            ));
            pb.set_message("Processing contents");
            */

            for (path, stable_packages) in
                self.load_contents_iter(Suite::Stable(None), architecture)?
            {
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

                let testing_packages = match testing_file_map.get(path_to_test.as_str()) {
                    Some(packages) => packages,
                    None => continue,
                };

                let stable_packages_set: HashSet<&str> =
                    HashSet::from_iter(stable_packages.iter().map(|v| v.as_str()));
                let testing_packages_set: HashSet<&str> =
                    HashSet::from_iter(testing_packages.iter().map(|v| v.as_str()));
                if stable_packages_set != testing_packages_set {
                    println!(
                        "{}: {} => {}: {:?} vs {:?}",
                        architecture, path, path_to_test, stable_packages, testing_packages,
                    );
                } else if self.options.only_files_moved {
                    println!(
                        "{}: {} => {}: {:?}",
                        architecture, path, path_to_test, stable_packages,
                    );
                } else {
                    info!(
                        "Renamed {} to {} (packages {:?})",
                        path, path_to_test, testing_packages,
                    );
                }
            }
        }

        Ok(())
    }
}
