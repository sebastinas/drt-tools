// Copyright 2022-2023 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::iter::FusedIterator;
use std::path::PathBuf;

use anyhow::Result;
use assorted_debian_utils::architectures::{Architecture, RELEASE_ARCHITECTURES};
use assorted_debian_utils::archive::Suite;
use async_trait::async_trait;
use clap::Parser;
use log::{debug, info, trace, warn};
use smallvec::SmallVec;

use crate::config::{self, CacheEntries};
use crate::Command;

type SmallString = smartstring::alias::String;

const SKIP: [&str; 6] = [
    "boot/",
    "etc/",
    "lib/modules/",
    "usr/src/",
    "usr/share/doc",
    "var/",
];

fn strip_section(package: &str) -> SmallString {
    package.split_once('/').map_or(package, |(_, p)| p).into()
}

fn compute_path_to_test_impl(path: &str) -> SmallString {
    if let Some(stripped) = path.strip_prefix("usr/") {
        stripped.into()
    } else {
        format!("usr/{}", path).into()
    }
}

fn compute_path_to_test(path: impl AsRef<str>) -> SmallString {
    compute_path_to_test_impl(path.as_ref())
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

pub(crate) struct UsrMerged<'a> {
    cache: &'a config::Cache,
    options: UsrMergedOptions,
}

struct LoadIterator {
    reader: BufReader<File>,
    suite: Suite,
    arch: Architecture,
    no_skip: bool,
}

impl Iterator for LoadIterator {
    // if there is a file in more than one package, the most common case are two packages
    type Item = (SmallString, SmallVec<[SmallString; 2]>);

    fn next(&mut self) -> Option<Self::Item> {
        let mut line = String::new();
        while let Ok(size) = self.reader.read_line({
            line.clear();
            &mut line
        }) {
            if size == 0 {
                // reached EOF
                return None;
            }
            let line = line
                .strip_suffix("\r\n")
                .or(line.strip_suffix('\n'))
                .unwrap_or(&line);
            trace!("{}/{}: processing: {}", self.suite, self.arch, line);

            let mut split = line.split_whitespace();
            let (path, packages) = match (split.next(), split.next()) {
                (Some(path), Some(packages)) => (path, packages),
                _ => {
                    warn!("Unable to process line: {}", line);
                    continue;
                }
            };

            // skip some well-known locations which should not be an issue: boot/, usr/etc/, usr/lib/modules/, ...
            if !self.no_skip && SKIP.into_iter().any(|prefix| path.starts_with(prefix)) {
                debug!("Skipping '{}'", path);
                continue;
            }

            return Some((
                path.into(),
                packages.split(',').map(strip_section).collect(),
            ));
        }
        // Error
        None
    }
}

impl FusedIterator for LoadIterator {}

fn load_contents_iter(
    suite: Suite,
    arch: Architecture,
    path: PathBuf,
    no_skip: bool,
) -> Result<LoadIterator> {
    debug!("Processing contents for {} on {}: {:?}", suite, arch, path);

    let reader = BufReader::new(File::open(path)?);
    Ok(LoadIterator {
        reader,
        suite,
        arch,
        no_skip,
    })
}

impl<'a> UsrMerged<'a> {
    pub(crate) fn new(cache: &'a config::Cache, options: UsrMergedOptions) -> Self {
        Self { cache, options }
    }

    fn load_contents_iter(&self, suite: Suite, arch: Architecture) -> Result<LoadIterator> {
        for (architecture, path) in self.cache.get_content_paths(suite)? {
            if architecture != arch {
                continue;
            }

            return load_contents_iter(suite, arch, path, self.options.no_skip);
        }

        unreachable!("This will never be reached.");
    }

    fn load_contents(
        &self,
        suite: Suite,
        arch: Architecture,
    ) -> Result<HashMap<SmallString, SmallVec<[SmallString; 2]>>> {
        for (architecture, path) in self.cache.get_content_paths(suite)? {
            if architecture != arch {
                continue;
            }

            return Ok(HashMap::from_iter(load_contents_iter(
                suite,
                arch,
                path,
                self.options.no_skip,
            )?));
        }

        unreachable!("This will never be reached.");
    }
}

#[async_trait]
impl Command for UsrMerged<'_> {
    async fn run(&self) -> Result<()> {
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
                    testing_file_map.get(&path_to_test),
                    testing_all_file_map.get(&path_to_test),
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

                let testing_packages_set_original_path =
                    match (testing_file_map.get(&path), testing_all_file_map.get(&path)) {
                        (None, None) => {
                            debug!("{}: {} not found", architecture, path_to_test);
                            HashSet::new()
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

    fn downloads(&self) -> Vec<CacheEntries> {
        [
            CacheEntries::Contents(Suite::Stable(None)),
            CacheEntries::Contents(Suite::Testing(None)),
        ]
        .into()
    }
}
