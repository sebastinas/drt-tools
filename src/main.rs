// Copyright 2021-2024 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::{Context, Result};
use assorted_debian_utils::{archive::SuiteOrCodename, wb::WBArchitecture};
use async_trait::async_trait;
use clap::{Parser, Subcommand};
use config::{CacheEntries, CacheState};
use log::trace;

mod binnmu_buildinfo;
pub(crate) mod config;
mod grep_excuses;
mod nmu_eso;
mod nmu_transition;
mod nmu_versionskew;
mod process_excuses;
pub(crate) mod source_packages;
pub(crate) mod udd_bugs;
pub(crate) mod utils;

use binnmu_buildinfo::{BinNMUBuildinfo, BinNMUBuildinfoOptions};
use grep_excuses::{GrepExcuses, GrepExcusesOptions};
use nmu_eso::{NMUOutdatedBuiltUsing, NMUOutdatedBuiltUsingOptions};
use nmu_transition::{NMUTransition, NMUTransitionOptions};
use nmu_versionskew::{NMUVersionSkew, NMUVersionSkewOptions};
use process_excuses::{ProcessExcuses, ProcessExcusesOptions};

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
    #[clap(flatten)]
    verbose: clap_verbosity_flag::Verbosity,
    /// Archive mirror
    #[clap(long, default_value = "https://deb.debian.org/debian")]
    mirror: String,
    /// buildd server
    #[clap(long, default_value = "wuiet.debian.org")]
    buildd: String,
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
    architecture: Option<Vec<WBArchitecture>>,
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
    /// Process current excuses.yaml and prepare a list of binNMUs required for
    /// testing migration and list of unblocks
    ///
    /// For unblocks, this command parses the current excuses and prepares a
    /// list of packages in testing-proposed-updates and packages that have been
    /// rebuilt in unstable but are blocked by the freeze.
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
    NMUTransition(NMUTransitionOptions),
    /// Prepare binNMUs based on a list of buildinfo files
    #[clap(name = "nmu-buildinfo")]
    NMUBuildinfo(BinNMUBuildinfoOptions),
    /// Grep excuses for a list of packages and/or maintainers
    ///
    /// This command checks `britney`'s excuses and autoremovals for the given
    /// packages and or maintainers.
    #[clap(name = "grep-excuses")]
    GrepExcuses(GrepExcusesOptions),
    /// Prepare binNMUs to rebuild for outdated Built-Using
    ///
    /// Based on the `Extra-Source-Only` flag, this command prepares and
    /// schedules binNMUs for packages with outdated `Built-Using` fields.
    #[clap(name = "nmu-eso")]
    NMUOutdatedBuiltUsing(NMUOutdatedBuiltUsingOptions),
    /// Prepare rebuilds for version skew in Multi-Arch: same packages
    #[clap(name = "nmu-version-skew")]
    NMUVersionSkew(NMUVersionSkewOptions),
}

pub(crate) trait Downloads {
    /// Cache entries that need to be downloaded and in fresh state.
    fn required_downloads(&self) -> Vec<CacheEntries> {
        Vec::new()
    }

    /// Cache entries that need to be downloaded
    fn downloads(&self) -> Vec<CacheEntries> {
        Vec::new()
    }
}

pub(crate) trait Command: Downloads {
    /// Execute the command
    fn run(&self) -> Result<()>;
}

#[async_trait]
pub(crate) trait AsyncCommand: Downloads {
    /// Execute the command
    async fn run(&self) -> Result<()>;
}

#[async_trait]
impl<T> AsyncCommand for T
where
    T: Command + Sync,
{
    async fn run(&self) -> Result<()> {
        <Self as Command>::run(self)
    }
}

async fn execute_command(
    cache: &config::Cache,
    command: &dyn AsyncCommand,
    force_processing: bool,
) -> Result<()> {
    let to_download = command.required_downloads();
    if !to_download.is_empty()
        && cache.download(&to_download).await? == CacheState::NoUpdate
        && !force_processing
    {
        trace!("all files are up-to-date; nothing to do");
        return Ok(());
    }

    cache.download(&command.downloads()).await?;
    command.run().await
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts = DrtToolsOptions::parse();

    stderrlog::new()
        .verbosity(opts.base_options.verbose.log_level_filter())
        .init()
        .with_context(|| "Failed to initialize `stderrlog`.")?;
    trace!("base options {:?}", opts.base_options);
    trace!("command: {:?}", opts.command);

    let cache =
        config::Cache::new(opts.base_options.force_download, &opts.base_options.mirror).await?;
    let command: Box<dyn AsyncCommand> =
        match opts.command {
            DrtToolsCommands::ProcessExcuses(pe_opts) => {
                Box::new(ProcessExcuses::new(&cache, &opts.base_options, pe_opts))
            }
            DrtToolsCommands::NMUTransition(pbm_opts) => {
                Box::new(NMUTransition::new(&cache, &opts.base_options, pbm_opts))
            }
            DrtToolsCommands::NMUBuildinfo(bb_opts) => {
                Box::new(BinNMUBuildinfo::new(&cache, &opts.base_options, bb_opts))
            }
            DrtToolsCommands::GrepExcuses(ge_opts) => Box::new(GrepExcuses::new(&cache, ge_opts)),
            DrtToolsCommands::NMUOutdatedBuiltUsing(eso_opts) => Box::new(
                NMUOutdatedBuiltUsing::new(&cache, &opts.base_options, eso_opts),
            ),
            DrtToolsCommands::NMUVersionSkew(vs_opts) => {
                Box::new(NMUVersionSkew::new(&cache, &opts.base_options, vs_opts))
            }
        };
    execute_command(&cache, command.as_ref(), opts.base_options.force_processing).await
}
