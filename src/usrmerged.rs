// Copyright 2022 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};

use anyhow::Result;
use assorted_debian_utils::architectures::{Architecture, RELEASE_ARCHITECTURES};
use assorted_debian_utils::archive::Suite;
use clap::Parser;
use log::{debug, info, trace, warn};
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

fn compute_path_to_test(path: impl AsRef<str>) -> String {
    if let Some(stripped) = path.as_ref().strip_prefix("usr/") {
        stripped.into()
    } else {
        format!("usr/{}", path.as_ref())
    }
}

#[derive(Debug, Parser)]
pub(crate) struct UsrMergedOptions {
    /// Also include files that only moved between / and /usr but stayed in the same package
    #[clap(long)]
    include_moved_in_package: bool,
    /// Do not skip some paths known to not be affected
    #[clap(long)]
    no_skip: bool,
}

pub(crate) struct UsrMerged {
    cache: config::Cache,
    options: UsrMergedOptions,
}

impl UsrMerged {
    pub(crate) fn new(base_options: BaseOptions, options: UsrMergedOptions) -> Result<Self> {
        Ok(Self {
            cache: config::Cache::new(base_options.force_download, &base_options.mirror)?,
            options,
        })
    }

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

            debug!(
                "Processing contents for {} on {}: {:?}",
                suite, architecture, path
            );

            let no_skip = self.options.no_skip;
            let reader = BufReader::new(File::open(path)?);
            return Ok(Box::new(reader.lines().filter_map(move |line| {
                let line = match line {
                    Ok(line) => line,
                    _ => {
                        return None;
                    }
                };
                trace!("Processing: {}", line);

                let mut split = line.split_whitespace();
                let (path, packages) = match (split.next(), split.next()) {
                    (Some(path), Some(packages)) => (path, packages),
                    _ => {
                        warn!("Unable to process line: {}", line);
                        return None;
                    }
                };

                // skip some well-known locations which should not be an issue: boot/, usr/etc/, usr/lib/modules/, ...
                const SKIP: [&str; 6] = [
                    "boot/",
                    "etc/",
                    "lib/modules/",
                    "usr/src/",
                    "usr/share/doc",
                    "var/",
                ];
                if !no_skip && SKIP.into_iter().any(|prefix| path.starts_with(prefix)) {
                    debug!("Skipping {}", path);
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

    pub(crate) async fn run(self) -> Result<()> {
        self.download_to_cache().await?;

        // Check if file from stable on $architecture moved to other
        // packages on testing on $architecture | all. If architecture ==
        // all, this will check all -> all.
        let mut testing_all_file_map =
            self.load_contents(Suite::Testing(None), Architecture::All)?;
        for architecture in RELEASE_ARCHITECTURES
            .into_iter()
            .chain([Architecture::All].into_iter())
        {
            let testing_file_map = if architecture != Architecture::All {
                self.load_contents(Suite::Testing(None), architecture)?
            } else {
                HashMap::new()
            };

            for (path, stable_packages) in
                self.load_contents_iter(Suite::Stable(None), architecture)?
            {
                let path_to_test = compute_path_to_test(&path);
                debug!(
                    "{}: processing {} - checking for {}",
                    architecture, path, path_to_test
                );

                let testing_packages_set = match (
                    testing_file_map.get(path_to_test.as_str()),
                    testing_all_file_map.get(path_to_test.as_str()),
                ) {
                    (None, None) => {
                        debug!("{}: {} not found", architecture, path_to_test);
                        continue;
                    }
                    (None, Some(packages)) | (Some(packages), None) => {
                        HashSet::from_iter(packages.iter().map(|v| v.as_str()))
                    }
                    (Some(arch_packages), Some(all_packages)) => HashSet::from_iter(
                        arch_packages
                            .iter()
                            .chain(all_packages.iter())
                            .map(|v| v.as_str()),
                    ),
                };

                let testing_packages_set_original_path = match (
                    testing_file_map.get(path.as_str()),
                    testing_all_file_map.get(path.as_str()),
                ) {
                    (None, None) => {
                        debug!("{}: {} not found", architecture, path_to_test);
                        continue;
                    }
                    (None, Some(packages)) | (Some(packages), None) => {
                        HashSet::from_iter(packages.iter().map(|v| v.as_str()))
                    }
                    (Some(arch_packages), Some(all_packages)) => HashSet::from_iter(
                        arch_packages
                            .iter()
                            .chain(all_packages.iter())
                            .map(|v| v.as_str()),
                    ),
                };

                let stable_packages_set: HashSet<&str> =
                    HashSet::from_iter(stable_packages.iter().map(|v| v.as_str()));
                if stable_packages_set != testing_packages_set {
                    if stable_packages_set == testing_packages_set_original_path {
                        println!(
                            "also-in-other-package: {}: {} {:?} vs {} {:?}",
                            architecture,
                            path,
                            stable_packages_set,
                            path_to_test,
                            testing_packages_set,
                        );
                    } else {
                        println!(
                            "moved: {}: {} => {}: {:?} vs {:?}",
                            architecture,
                            path,
                            path_to_test,
                            stable_packages_set,
                            testing_packages_set,
                        );
                    }
                } else if self.options.include_moved_in_package {
                    println!(
                        "moved-in-package: {}: {} => {}: {:?}",
                        architecture, path, path_to_test, stable_packages_set,
                    );
                } else {
                    info!(
                        "Renamed {} to {} (packages {:?})",
                        path, path_to_test, testing_packages_set,
                    );
                }
            }
        }
        testing_all_file_map.clear();

        // Check if file from stable on all moved to other
        // packages on testing on $architecture for all architectures except all.
        for testing_architecture in RELEASE_ARCHITECTURES.into_iter() {
            let testing_file_map =
                self.load_contents(Suite::Testing(None), testing_architecture)?;
            for (path, stable_packages) in
                self.load_contents_iter(Suite::Stable(None), Architecture::All)?
            {
                let path_to_test = compute_path_to_test(&path);
                debug!(
                    "{} -> {}: processing {} - checking for {}",
                    Architecture::All,
                    testing_architecture,
                    path,
                    path_to_test
                );

                let testing_packages_set = match testing_file_map.get(path_to_test.as_str()) {
                    None => {
                        debug!("{}: {} not found", Architecture::All, path_to_test);
                        continue;
                    }
                    Some(packages) => HashSet::from_iter(packages.iter().map(|v| v.as_str())),
                };
                let testing_packages_set_original_path = match testing_file_map.get(path.as_str()) {
                    None => {
                        debug!("{}: {} not found", Architecture::All, path_to_test);
                        continue;
                    }
                    Some(packages) => HashSet::from_iter(packages.iter().map(|v| v.as_str())),
                };

                let stable_packages_set: HashSet<&str> =
                    HashSet::from_iter(stable_packages.iter().map(|v| v.as_str()));
                if stable_packages_set != testing_packages_set {
                    if stable_packages_set == testing_packages_set_original_path {
                        println!(
                            "also-in-other-package: {}: {} {:?} vs {} {:?}",
                            Architecture::All,
                            path,
                            stable_packages_set,
                            path_to_test,
                            testing_packages_set,
                        );
                    } else {
                        println!(
                            "moved: {} -> {}: {} => {}: {:?} vs {:?}",
                            Architecture::All,
                            testing_architecture,
                            path,
                            path_to_test,
                            stable_packages_set,
                            testing_packages_set,
                        );
                    }
                } else if self.options.include_moved_in_package {
                    println!(
                        "moved-in-package: {} -> {}: {} => {}: {:?}",
                        Architecture::All,
                        testing_architecture,
                        path,
                        path_to_test,
                        stable_packages_set,
                    );
                } else {
                    info!(
                        "Renamed {} to {} (packages {:?})",
                        path, path_to_test, testing_packages_set,
                    );
                }
            }
        }

        Ok(())
    }
}
