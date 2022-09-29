// Copyright 2022 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::{collections::HashSet, fs::File};

use anyhow::{anyhow, Result};
use assorted_debian_utils::archive::Codename;
use assorted_debian_utils::rfc822_like;
use assorted_debian_utils::version::PackageVersion;
use assorted_debian_utils::{
    architectures::Architecture,
    buildinfo::{self, Buildinfo},
    wb::{BinNMU, SourceSpecifier, WBCommand, WBCommandBuilder},
};
use clap::Parser;
use indicatif::{ProgressBar, ProgressIterator};
use serde::Deserialize;

use crate::config::default_progress_style;
use crate::udd_bugs::{load_bugs_from_reader, UDDBugs};
use crate::{
    config::{Cache, CacheEntries, CacheState},
    source_packages::SourcePackages,
    BaseOptions, BinNMUsOptions,
};

#[derive(Deserialize, Debug, Eq, PartialEq)]
#[serde(rename_all = "PascalCase")]
struct BinaryPackage {
    source: Option<String>,
    package: String,
    version: PackageVersion,
}

#[derive(Debug, Parser)]
pub(crate) struct BinNMUBuildinfoOptions {
    #[clap(flatten)]
    binnmu_options: BinNMUsOptions,
    /// Input files
    inputs: Vec<PathBuf>,
}

pub(crate) struct BinNMUBuildinfo {
    cache: Cache,
    base_options: BaseOptions,
    options: BinNMUBuildinfoOptions,
}

impl BinNMUBuildinfo {
    pub(crate) fn new(base_options: BaseOptions, options: BinNMUBuildinfoOptions) -> Result<Self> {
        Ok(Self {
            cache: Cache::new(base_options.force_download, &base_options.mirror)?,
            base_options,
            options,
        })
    }

    async fn download_to_cache(&self) -> Result<CacheState> {
        self.cache
            .download(&[
                CacheEntries::Packages,
                CacheEntries::FTBFSBugs(self.options.binnmu_options.suite.into()),
            ])
            .await
    }

    fn parse_packages(path: impl AsRef<Path>) -> Result<HashSet<(String, PackageVersion)>> {
        // read Package file
        let binary_packages: Vec<BinaryPackage> = rfc822_like::from_file(path.as_ref())?;
        let pb = ProgressBar::new(binary_packages.len() as u64);
        pb.set_style(default_progress_style().template(
            "{msg}: {spinner:.green} [{wide_bar:.cyan/blue}] {pos}/{len} ({per_sec}, {eta})",
        )?);
        pb.set_message(format!("Processing {}", path.as_ref().display()));

        Ok(binary_packages
            .into_iter()
            .progress_with(pb)
            .map(|binary_package| {
                if let Some(source_package) = &binary_package.source {
                    (
                        source_package.split_whitespace().next().unwrap().into(),
                        binary_package.version,
                    )
                } else {
                    // no Source set, so Source == Package
                    (binary_package.package, binary_package.version)
                }
            })
            .collect())
    }

    fn process(
        &self,
        buildinfo: Buildinfo,
        source_packages: &SourcePackages,
        source_versions: &HashMap<String, PackageVersion>,
        ftbfs_bugs: &UDDBugs,
    ) -> Result<WBCommand> {
        let mut source_split = buildinfo.source.split_whitespace();
        let source_package = source_split.next().unwrap();

        let architectures: Vec<Architecture> = buildinfo
            .architecture
            .into_iter()
            .filter(|arch| *arch != Architecture::All && *arch != Architecture::Source)
            .collect();
        if architectures.is_empty() {
            return Err(anyhow!("no binNMU-able architecture"));
        }

        match source_versions.get(source_package) {
            Some(version) => {
                if version > &buildinfo.version {
                    return Err(anyhow!("newer version in archive"));
                }
            }
            None => return Err(anyhow!("removed from the archive")),
        }

        if ftbfs_bugs.bugs_for_source(source_package).is_some() {
            return Err(anyhow!("skipping due to FTBFS bugs"));
        }

        let mut source = SourceSpecifier::new(source_package);
        let version = buildinfo.version.without_binnmu_version();
        source
            .with_version(&version)
            .with_suite(&self.options.binnmu_options.suite);
        if !source_packages.is_ma_same(source_package) {
            // binNMU only on the architecture if no MA: same binary packages
            source.with_archive_architectures(&architectures);
        }

        let mut binnmu = BinNMU::new(&source, &self.options.binnmu_options.message)?;
        if let Some(bp) = self.options.binnmu_options.build_priority {
            binnmu.with_build_priority(bp);
        }
        if let Some(dw) = &self.options.binnmu_options.dep_wait {
            binnmu.with_dependency_wait(dw);
        }
        if let Some(extra_depends) = &self.options.binnmu_options.extra_depends {
            binnmu.with_extra_depends(extra_depends);
        }
        Ok(binnmu.build())
    }

    fn process_path(
        &self,
        path: impl AsRef<Path>,
        source_packages: &SourcePackages,
        source_versions: &HashMap<String, PackageVersion>,
        ftbfs_bugs: &UDDBugs,
    ) -> Result<HashSet<WBCommand>> {
        let mut ret = HashSet::new();
        let path = path.as_ref();
        if path.is_dir() {
            for path in path.read_dir()? {
                ret.extend(
                    self.process_path(path?.path(), source_packages, source_versions, ftbfs_bugs)
                        .unwrap_or_default(),
                );
            }
        } else {
            let data = strip_signature(BufReader::new(File::open(path)?))?;
            match buildinfo::from_reader(data.as_ref()) {
                Err(e) => {
                    println!("# skipping {}: {}", path.display(), e);
                }
                Ok(bi) => match self.process(bi, source_packages, source_versions, ftbfs_bugs) {
                    Err(e) => {
                        println!("# skipping {}: {}", path.display(), e,);
                    }
                    Ok(command) => {
                        ret.insert(command);
                    }
                },
            }
        }
        Ok(ret)
    }

    fn load_bugs(&self) -> Result<UDDBugs> {
        load_bugs_from_reader(self.cache.get_cache_bufreader(format!(
            "udd-ftbfs-bugs-{}.yaml",
            Codename::from(self.options.binnmu_options.suite)
        ))?)
    }

    pub(crate) async fn run(self) -> Result<()> {
        self.download_to_cache().await?;

        // store latest version of all source packages
        let mut source_versions: HashMap<String, PackageVersion> = HashMap::new();
        for path in self.cache.get_package_paths(true)? {
            for (source, version) in Self::parse_packages(path)?.into_iter() {
                match source_versions.get_mut(&source) {
                    Some(old_ver) => {
                        if version > *old_ver {
                            *old_ver = version;
                        }
                    }
                    None => {
                        source_versions.insert(source, version);
                    }
                }
            }
        }
        let source_packages = SourcePackages::new(&self.cache.get_package_paths(false)?)?;

        let ftbfs_bugs = if !self.base_options.force_processing {
            self.load_bugs()?
        } else {
            UDDBugs::new(vec![])
        };

        let mut wb_commands = HashSet::new();
        // iterate over all buildinfo files
        for filename in &self.options.inputs {
            wb_commands.extend(
                self.process_path(filename, &source_packages, &source_versions, &ftbfs_bugs)
                    .unwrap_or_default(),
            );
        }

        for commands in wb_commands {
            println!("{}", commands);
            if !self.base_options.dry_run {
                commands.execute()?;
            }
        }

        Ok(())
    }
}

// Strip the signature from a buildinfo file without verifying it
fn strip_signature(input: impl BufRead) -> Result<Vec<u8>> {
    let mut data = vec![];
    for line in input.lines().skip_while(|rline| {
        // Skip until the beginning of a buildinfo file
        rline
            .as_ref()
            .map(|line| !line.starts_with("Format: "))
            .unwrap_or(true)
    }) {
        let line = match line {
            Ok(v) => v,
            Err(_) => {
                break;
            }
        };

        // Read until beginning of the signature block
        if line.starts_with("-----BEGIN") {
            return Ok(data);
        }
        data.write_all(line.as_bytes())?;
        data.write_all(b"\n")?;
    }

    Ok(data)
}
