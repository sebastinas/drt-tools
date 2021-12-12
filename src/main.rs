// Copyright 2021 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::Result;
use structopt::StructOpt;

mod config;
pub(crate) mod downloader;
mod prepare_binnmus;
mod process_excuses;

use prepare_binnmus::{PrepareBinNMUs, PrepareBinNMUsOptions};
use process_excuses::{ProcessExcuses, ProcessExcusesOptions};

#[derive(Debug, StructOpt)]
pub(crate) struct BaseOptions {
    /// Force download of files
    #[structopt(long)]
    force_download: bool,
    /// Force processing
    #[structopt(short, long = "force")]
    force_processing: bool,
    /// Only print actions to perform without running any commends
    #[structopt(short = "n")]
    dry_run: bool,
}

#[derive(Debug, StructOpt)]
struct DrtToolsOptions {
    #[structopt(flatten)]
    base_options: BaseOptions,
    #[structopt(subcommand)]
    command: DrtToolsCommands,
}

#[derive(Debug, StructOpt)]
enum DrtToolsCommands {
    /// Process current excuses.yaml and prepare a list of binNMUs to perform testing migration
    ProcessExcuses(ProcessExcusesOptions),
    /// Prepare binNMUs for a transition
    #[structopt(name = "prepare-binNMUs")]
    PrepareBinNMUs(PrepareBinNMUsOptions),
}

fn main() -> Result<()> {
    let opts = DrtToolsOptions::from_args();
    match opts.command {
        DrtToolsCommands::ProcessExcuses(pe_opts) => {
            let process_excuses = ProcessExcuses::new(opts.base_options, pe_opts)?;
            process_excuses.run()
        }
        DrtToolsCommands::PrepareBinNMUs(pbm_opts) => {
            let prepare_binnmus = PrepareBinNMUs::new(opts.base_options, pbm_opts);
            prepare_binnmus.run()
        }
    }
}
