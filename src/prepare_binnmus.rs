// Copyright 2021 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io::{self, BufRead, BufReader};
use std::path::PathBuf;
use std::{collections::HashMap, fs::File};

use anyhow::Result;
use clap::Parser;
use serde::Deserialize;

use crate::{
    config::{self, CacheEntries},
    BaseOptions,
};
use assorted_debian_utils::{
    architectures::Architecture,
    archive::{Codename, SuiteOrCodename},
    wb::{BinNMU, SourceSpecifier, WBCommandBuilder},
};

#[derive(Debug, Deserialize)]
struct UDDBug {
    id: u32,
    source: String,
}

#[derive(Debug, Parser)]
pub(crate) struct PrepareBinNMUsOptions {
    /// Message for binNMUs
    #[clap(short, long)]
    message: String,
    /// Set a build priority
    #[clap(long = "bp")]
    build_priority: Option<i32>,
    /// Set dependency-wait
    #[clap(long = "dw")]
    dep_wait: Option<String>,
    /// Set extra dependencies
    #[clap(long)]
    extra_depends: Option<String>,
    /// Set the suite
    #[clap(short, long, default_value = "unstable")]
    suite: SuiteOrCodename,
    /// Set architectures for binNMUs
    #[clap(short, long)]
    architecture: Option<Vec<Architecture>>,
    /// Input file
    #[clap(parse(from_os_str))]
    input: Option<PathBuf>,
}

pub(crate) struct PrepareBinNMUs {
    cache: config::Cache,
    base_options: BaseOptions,
    options: PrepareBinNMUsOptions,
}

impl PrepareBinNMUs {
    pub(crate) fn new(base_options: BaseOptions, options: PrepareBinNMUsOptions) -> Result<Self> {
        Ok(Self {
            cache: config::Cache::new(base_options.force_download)?,
            base_options,
            options,
        })
    }

    #[tokio::main]
    async fn download_to_cache(&self, codename: &Codename) -> Result<()> {
        self.cache
            .download(&[CacheEntries::FTBFSBugs(codename.to_string())])
            .await?;
        Ok(())
    }

    fn load_bugs(&self, codename: &Codename) -> HashMap<String, u32> {
        if let Err(e) = self.download_to_cache(codename) {
            println!(
                "# Unable to download FTBFS bugs for {}: {}",
                self.options.suite, e
            );
            return HashMap::new();
        }

        let bugs: Vec<UDDBug> = serde_yaml::from_reader(
            self.cache
                .get_cache_bufreader(format!("udd-ftbfs-bugs-{}.yaml", codename))
                .unwrap(),
        )
        .unwrap_or_default();
        bugs.into_iter().map(|bug| (bug.source, bug.id)).collect()
    }

    pub(crate) fn run(self) -> Result<()> {
        let codename: Codename = self.options.suite.clone().into();
        let ftbfs_bugs = self.load_bugs(&codename);

        let matcher = regex::Regex::new("([a-z0-9+.-]+)[ \t].* \\(?([0-9][^() \t]*)\\)?")?;

        let reader: Box<dyn BufRead> = match &self.options.input {
            None => Box::new(BufReader::new(io::stdin())),
            Some(filename) => Box::new(BufReader::new(File::open(filename)?)),
        };

        let mut wb_commands = Vec::new();
        for line in reader.lines() {
            if line.is_err() {
                break;
            }

            let line = line.unwrap();
            if let Some(capture) = matcher.captures(&line) {
                let package = capture.get(1);
                let version = capture.get(2);
                if package.is_none() || version.is_none() {
                    continue;
                }

                let source = package.unwrap().as_str();
                if let Some(bug) = ftbfs_bugs.get(source) {
                    println!("# Skipping {} due to FTBFS bug #{}", source, bug);
                    continue;
                }

                let mut source = SourceSpecifier::new(source);
                let version = version.unwrap().as_str().try_into()?;
                source
                    .with_version(&version)
                    .with_suite(&self.options.suite);
                if let Some(architectures) = &self.options.architecture {
                    source.with_archive_architectures(architectures);
                }

                let mut binnmu = BinNMU::new(&source, &self.options.message)?;
                if let Some(bp) = self.options.build_priority {
                    binnmu.with_build_priority(bp);
                }
                if let Some(dw) = &self.options.dep_wait {
                    binnmu.with_dependency_wait(dw);
                }
                if let Some(extra_depends) = &self.options.extra_depends {
                    binnmu.with_extra_depends(extra_depends);
                }
                wb_commands.push(binnmu.build())
            }
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
