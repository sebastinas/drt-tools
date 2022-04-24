// Copyright 2021-2022 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::Result;
use assorted_debian_utils::{architectures::Architecture, archive::SuiteOrCodename};
use clap::{Parser, Subcommand};

mod binnmu_buildinfo;
pub(crate) mod config;
mod grep_excuses;
mod nmu_eso;
mod prepare_binnmus;
mod process_excuses;
pub(crate) mod source_packages;

use binnmu_buildinfo::{BinNMUBuildinfo, BinNMUBuildinfoOptions};
use grep_excuses::{GrepExcuses, GrepExcusesOptions};
use nmu_eso::{NMUOutdatedBuiltUsing, NMUOutdatedBuiltUsingOptions};
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
pub(crate) struct BinNMUsOptions {
    /// Message for binNMUs
    #[clap(short, long)]
    message: String,
    /// Build priority. If specified, the binNMUs are scheduled with the given build priority. Builds with a positive priority will be built earlier.
    #[clap(long = "bp")]
    build_priority: Option<i32>,
    /// Dependency-wait. If specified, the builds will wait until the given dependency relation is satisfied.
    #[clap(long = "dw")]
    dep_wait: Option<String>,
    /// Extra dependencies. If specified, the given dependency will be installed during the build.
    #[clap(long)]
    extra_depends: Option<String>,
    /// Suite for binNMUs.
    #[clap(short, long, default_value = "unstable")]
    suite: SuiteOrCodename,
    /// Set architectures for binNMUs. If no archictures are specified, the binNMUs are scheduled with ANY.
    #[clap(short, long)]
    architecture: Option<Vec<Architecture>>,
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
    /// Process current excuses.yaml and prepare a list of binNMUs required for testing migration.
    ProcessExcuses(ProcessExcusesOptions),
    /// Prepare and schedule binNMUs for a transition.
    ///
    /// This command expects a list of packages with their respective versions
    /// from ben. Each line should look like this:
    ///
    /// haskell-pandoc-citeproc    [build logs] (0.17.0.1-1)    ✘    ✘    ✘    ✘    ✘    ✘    ✘    ✘    ✘
    ///
    /// Note that any information from ben except the source package and the
    /// version are ignored. Per default, binNMUs are scheduled with ANY
    /// in unstable.
    ///
    /// The list of packages can be either given on the standard input or they
    /// are aread from a file.
    #[clap(name = "prepare-binNMUs")]
    PrepareBinNMUs(PrepareBinNMUsOptions),
    /// Prepare binNMUs based on a list of buildinfo files
    #[clap(name = "binNMU-buildinfo")]
    BinNMUBuildinfo(BinNMUBuildinfoOptions),
    /// Grep excuses
    #[clap(name = "grep-excuses")]
    GrepExcuses(GrepExcusesOptions),
    /// Prepare binNMUs to rebuild for outdated Built-Using
    #[clap(name = "nmu-eso")]
    NMUOutdatedBuiltUsing(NMUOutdatedBuiltUsingOptions),
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
        DrtToolsCommands::NMUOutdatedBuiltUsing(eso_opts) => {
            let nmu_eso = NMUOutdatedBuiltUsing::new(opts.base_options, eso_opts)?;
            nmu_eso.run()
        }
    }
}
