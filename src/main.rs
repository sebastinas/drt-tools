// Copyright 2021-2022 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::Result;
use assorted_debian_utils::{architectures::Architecture, archive::SuiteOrCodename};
use clap::{ArgAction, Parser, Subcommand};
use log::trace;

mod binnmu_buildinfo;
pub(crate) mod config;
mod grep_excuses;
mod nmu_eso;
mod prepare_binnmus;
mod process_excuses;
mod process_unblocks;
pub(crate) mod source_packages;
pub(crate) mod udd_bugs;
mod usrmerged;

use binnmu_buildinfo::{BinNMUBuildinfo, BinNMUBuildinfoOptions};
use grep_excuses::{GrepExcuses, GrepExcusesOptions};
use nmu_eso::{NMUOutdatedBuiltUsing, NMUOutdatedBuiltUsingOptions};
use prepare_binnmus::{PrepareBinNMUs, PrepareBinNMUsOptions};
use process_excuses::{ProcessExcuses, ProcessExcusesOptions};
use process_unblocks::ProcessUnblocks;
use usrmerged::{UsrMerged, UsrMergedOptions};

#[derive(Debug, Parser)]
pub(crate) struct BaseOptions {
    /// Force download of files
    #[clap(long)]
    force_download: bool,
    /// Force processing
    #[clap(short, long = "force")]
    force_processing: bool,
    /// Only print actions to perform without running any commands
    #[clap(short = 'n')]
    dry_run: bool,
    /// Quiet mode
    #[clap(short = 'q', long)]
    quiet: bool,
    /// Verbose mode (-v, -vv, -vvv, etc)
    #[clap(short = 'v', long, action(ArgAction::Count))]
    verbose: u8,
    /// Archive mirror
    #[clap(long, default_value = "https://deb.debian.org/debian")]
    mirror: String,
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
    /// are read from a file.
    #[clap(name = "nmu-transition")]
    PrepareBinNMUs(PrepareBinNMUsOptions),
    /// Prepare binNMUs based on a list of buildinfo files
    #[clap(name = "nmu-buildinfo")]
    BinNMUBuildinfo(BinNMUBuildinfoOptions),
    /// Grep excuses
    #[clap(name = "grep-excuses")]
    GrepExcuses(GrepExcusesOptions),
    /// Prepare binNMUs to rebuild for outdated Built-Using
    ///
    /// Based on
    /// `https://ftp-master.debian.org/users/ansgar/outdated-built-using.txt`,
    /// this command prepares and schedules binNMUs for packages with outdated
    /// `Built-Using` fields.
    #[clap(name = "nmu-eso")]
    NMUOutdatedBuiltUsing(NMUOutdatedBuiltUsingOptions),
    /// Check state of /usr-merged bugs
    ///
    /// Currently, a moratorium is in place that forbids files to move from
    /// `/{bin,lib}` to `/usr/{bin,lib}` or vice-versa if the file moves from
    /// one binary package to another at the same time. This tool tries to find
    /// all occurrences of all files violating this rule between stable and
    /// testing.
    ///
    /// Note that this subcommand requires at least 2 GB of available RAM.
    #[clap(name = "usrmerged")]
    UsrMerged(UsrMergedOptions),
    /// Prepare a list of unblocks for binNMUs in tpu
    ProcessUnblocks,
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts = DrtToolsOptions::parse();

    stderrlog::new()
        .quiet(opts.base_options.quiet)
        .verbosity(opts.base_options.verbose as usize)
        .init()?;
    trace!("base options {:?}", opts.base_options);
    trace!("command: {:?}", opts.command);

    match opts.command {
        DrtToolsCommands::ProcessExcuses(pe_opts) => {
            let process_excuses = ProcessExcuses::new(opts.base_options, pe_opts)?;
            process_excuses.run().await
        }
        DrtToolsCommands::PrepareBinNMUs(pbm_opts) => {
            let prepare_binnmus = PrepareBinNMUs::new(opts.base_options, pbm_opts)?;
            prepare_binnmus.run().await
        }
        DrtToolsCommands::BinNMUBuildinfo(bb_opts) => {
            let binnmus_buildinfo = BinNMUBuildinfo::new(opts.base_options, bb_opts)?;
            binnmus_buildinfo.run().await
        }
        DrtToolsCommands::GrepExcuses(ge_opts) => {
            let grep_excuses = GrepExcuses::new(opts.base_options, ge_opts)?;
            grep_excuses.run().await
        }
        DrtToolsCommands::NMUOutdatedBuiltUsing(eso_opts) => {
            let nmu_eso = NMUOutdatedBuiltUsing::new(opts.base_options, eso_opts)?;
            nmu_eso.run().await
        }
        DrtToolsCommands::UsrMerged(um_opts) => {
            let usr_merged = UsrMerged::new(opts.base_options, um_opts)?;
            usr_merged.run().await
        }
        DrtToolsCommands::ProcessUnblocks => {
            let process_unblocks = ProcessUnblocks::new(opts.base_options)?;
            process_unblocks.run().await
        }
    }
}
