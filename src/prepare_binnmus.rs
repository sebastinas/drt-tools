// Copyright 2021 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io::{self, BufRead};

use anyhow::Result;
use structopt::StructOpt;

use assorted_debian_utils::{
    architectures::Architecture,
    wb::{BinNMU, SourceSpecifier, WBCommandBuilder},
};

#[derive(Debug, StructOpt)]
pub(crate) struct PrepareBinNMUsOptions {
    #[structopt(short, long)]
    message: String,
    #[structopt(long = "bp")]
    build_priority: Option<i32>,
    #[structopt(long = "dw")]
    dep_wait: Option<String>,
    #[structopt(long)]
    extra_depends: Option<String>,
    #[structopt(short, long, default_value = "unstable")]
    suite: String,
    #[structopt(short, long)]
    architecture: Option<Vec<Architecture>>,
    #[structopt(long)]
    schedule: bool,
}

pub(crate) struct PrepareBinNMUs {
    options: PrepareBinNMUsOptions,
}

impl PrepareBinNMUs {
    pub(crate) fn new(options: PrepareBinNMUsOptions) -> Self {
        Self { options }
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
            if self.options.schedule {
                commands.execute()?;
            }
        }

        Ok(())
    }
}
