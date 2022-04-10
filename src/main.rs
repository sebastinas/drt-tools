// Copyright 2021 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::Result;
use clap::{Parser, Subcommand};

mod binnmu_buildinfo;
pub(crate) mod config;
mod grep_excuses;
mod prepare_binnmus;
mod process_excuses;
pub(crate) mod source_packages;

use binnmu_buildinfo::{BinNMUBuildinfo, BinNMUBuildinfoOptions};
use grep_excuses::{GrepExcuses, GrepExcusesOptions};
use prepare_binnmus::{PrepareBinNMUs, PrepareBinNMUsOptions};
use process_excuses::{ProcessExcuses, ProcessExcusesOptions};

#[derive(Debug, Parser)]
pub(crate) struct BaseOptions {
    /// Force download of files
    #[clap(long)]
    force_download: bool,
    /// Force processing
    #[clap(short, long = "force")]
    force_processing: bool,
    /// Only print actions to perform without running any commends
    #[clap(short = 'n')]
    dry_run: bool,
}

#[derive(Debug, Parser)]
struct DrtToolsOptions {
    #[clap(flatten)]
    base_options: BaseOptions,
    #[clap(subcommand)]
    command: DrtToolsCommands,
}

#[derive(Debug, Subcommand)]
enum DrtToolsCommands {
    /// Process current excuses.yaml and prepare a list of binNMUs to perform testing migration
    ProcessExcuses(ProcessExcusesOptions),
    /// Prepare binNMUs for a transition
    #[clap(name = "prepare-binNMUs")]
    PrepareBinNMUs(PrepareBinNMUsOptions),
    /// Prepare binNMUs based on a list of buildinfo files
    #[clap(name = "binNMU-buildinfo")]
    BinNMUBuildinfo(BinNMUBuildinfoOptions),
    /// Grep excuses
    #[clap(name = "grep-excuses")]
    GrepExcuses(GrepExcusesOptions),
}

fn main() -> Result<()> {
    let opts = DrtToolsOptions::parse();
    match opts.command {
        DrtToolsCommands::ProcessExcuses(pe_opts) => {
            let process_excuses = ProcessExcuses::new(opts.base_options, pe_opts)?;
            process_excuses.run()
        }
        DrtToolsCommands::PrepareBinNMUs(pbm_opts) => {
            let prepare_binnmus = PrepareBinNMUs::new(opts.base_options, pbm_opts)?;
            prepare_binnmus.run()
        }
        DrtToolsCommands::BinNMUBuildinfo(bb_opts) => {
            let binnmus_buildinfo = BinNMUBuildinfo::new(opts.base_options, bb_opts)?;
            binnmus_buildinfo.run()
        }
        DrtToolsCommands::GrepExcuses(ge_opts) => {
            let grep_excuses = GrepExcuses::new(opts.base_options, ge_opts)?;
            grep_excuses.run()
        }
    }
}
