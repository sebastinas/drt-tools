// Copyright 2022-2023 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{BufRead, BufReader, Write},
    path::Path,
};

use anyhow::{Context, Result, anyhow};
use assorted_debian_utils::{
    architectures::Architecture,
    archive::SuiteOrCodename,
    buildinfo::{self, Buildinfo},
    package::{PackageName, VersionedPackage},
    rfc822_like,
    version::PackageVersion,
    wb::{BinNMU, SourceSpecifier, WBCommand, WBCommandBuilder},
};
use async_trait::async_trait;
use indicatif::{ProgressBar, ProgressIterator};

use crate::{
    AsyncCommand, Downloads,
    cli::{BaseOptions, BinNMUBuildinfoOptions},
    config::{Cache, CacheEntries, CachePaths, default_progress_style, default_progress_template},
    source_packages::{BinaryPackage, SourcePackages},
    udd_bugs::UDDBugs,
    utils::execute_wb_commands,
};

pub(crate) struct BinNMUBuildinfo<'a> {
    cache: &'a Cache,
    base_options: &'a BaseOptions,
    options: BinNMUBuildinfoOptions,
}

impl<'a> BinNMUBuildinfo<'a> {
    pub(crate) fn new(
        cache: &'a Cache,
        base_options: &'a BaseOptions,
        options: BinNMUBuildinfoOptions,
    ) -> Self {
        Self {
            cache,
            base_options,
            options,
        }
    }

    fn parse_packages(path: impl AsRef<Path>) -> Result<HashSet<VersionedPackage>> {
        // read Package file
        let binary_packages: Vec<BinaryPackage> = rfc822_like::from_file(path.as_ref())
            .with_context(|| {
                format!("Failed to parse package file '{}'", path.as_ref().display())
            })?;
        let pb = ProgressBar::new(binary_packages.len() as u64);
        pb.set_style(default_progress_style().template(default_progress_template())?);
        pb.set_message(format!("Processing {}", path.as_ref().display()));

        Ok(binary_packages
            .into_iter()
            .progress_with(pb)
            .map(|binary_package| binary_package.source_package())
            .collect())
    }

    fn process(
        &self,
        buildinfo: Buildinfo,
        source_packages: &SourcePackages,
        source_versions: &HashMap<PackageName, PackageVersion>,
        ftbfs_bugs: &UDDBugs,
    ) -> Result<WBCommand> {
        let architectures: Vec<Architecture> = buildinfo
            .architecture
            .into_iter()
            .filter(|arch| *arch != Architecture::All && *arch != Architecture::Source)
            .collect();
        if architectures.is_empty() {
            return Err(anyhow!("no binNMU-able architecture"));
        }

        match source_versions.get(&buildinfo.source) {
            Some(version) => {
                if version > &buildinfo.version {
                    return Err(anyhow!("newer version in archive"));
                }
            }
            None => return Err(anyhow!("removed from the archive")),
        }

        if ftbfs_bugs.bugs_for_source(&buildinfo.source).is_some() {
            return Err(anyhow!("skipping due to FTBFS bugs"));
        }

        let mut source = SourceSpecifier::new(&buildinfo.source);
        let version = buildinfo.version.without_binnmu_version();
        source
            .with_version(&version)
            .with_suite(self.options.binnmu_options.suite);
        if !source_packages.is_ma_same(&buildinfo.source) {
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
        source_versions: &HashMap<PackageName, PackageVersion>,
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
}

#[async_trait]
impl AsyncCommand for BinNMUBuildinfo<'_> {
    async fn run(&self) -> Result<()> {
        // store latest version of all source packages
        let mut source_versions = HashMap::new();
        for path in self
            .cache
            .get_package_paths(SuiteOrCodename::UNSTABLE, true)?
        {
            for VersionedPackage {
                package: source,
                version,
            } in Self::parse_packages(path)?
            {
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
        let source_packages = SourcePackages::new(
            &self
                .cache
                .get_package_paths(SuiteOrCodename::UNSTABLE, false)?,
        )?;

        let ftbfs_bugs = if self.base_options.force_processing {
            UDDBugs::default()
        } else {
            UDDBugs::load_for_codename(self.cache, self.options.binnmu_options.suite)?
        };

        let mut wb_commands = HashSet::new();
        // iterate over all buildinfo files
        for filename in &self.options.inputs {
            if let Ok(commands) =
                self.process_path(filename, &source_packages, &source_versions, &ftbfs_bugs)
            {
                wb_commands.extend(commands);
            }
        }

        execute_wb_commands(wb_commands, self.base_options).await
    }
}

impl Downloads for BinNMUBuildinfo<'_> {
    fn downloads(&self) -> Vec<CacheEntries> {
        vec![
            CacheEntries::Packages(self.options.binnmu_options.suite),
            CacheEntries::FTBFSBugs(self.options.binnmu_options.suite),
        ]
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
        let Ok(line) = line else {
            break;
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
