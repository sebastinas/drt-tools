// Copyright 2021 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io::{self, BufRead};

use anyhow::Result;
use structopt::StructOpt;

use crate::BaseOptions;
use assorted_debian_utils::{
    architectures::Architecture,
    wb::{BinNMU, SourceSpecifier, WBCommandBuilder},
};

#[derive(Debug, StructOpt)]
pub(crate) struct PrepareBinNMUsOptions {
    /// Message for binNMUs
    #[structopt(short, long)]
    message: String,
    /// Set a build priority
    #[structopt(long = "bp")]
    build_priority: Option<i32>,
    /// Set dependency-wait
    #[structopt(long = "dw")]
    dep_wait: Option<String>,
    /// Set extra dependencies
    #[structopt(long)]
    extra_depends: Option<String>,
    /// Set the suite
    #[structopt(short, long, default_value = "unstable")]
    suite: String,
    /// Set architectures for binNMUs
    #[structopt(short, long)]
    architecture: Option<Vec<Architecture>>,
}

pub(crate) struct PrepareBinNMUs {
    base_options: BaseOptions,
    options: PrepareBinNMUsOptions,
}

impl PrepareBinNMUs {
    pub(crate) fn new(base_options: BaseOptions, options: PrepareBinNMUsOptions) -> Self {
        Self {
            base_options,
            options,
        }
    }

    pub(crate) fn run(self) -> Result<()> {
        let matcher = regex::Regex::new("([a-z0-9+.-]+)[ \t].* \\(?([0-9][^() \t]*)\\)?")?;

        let mut wb_commands = Vec::new();
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
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

                let mut source = SourceSpecifier::new(package.unwrap().as_str());
                source
                    .with_version(version.unwrap().as_str())
                    .with_suite(&self.options.suite);
                if let Some(architectures) = &self.options.architecture {
                    source.with_archive_architectures(architectures);
                }

                let mut binnmu = BinNMU::new(&source, &self.options.message);
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
