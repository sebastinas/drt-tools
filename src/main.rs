// Copyright 2021-2024 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

#![warn(clippy::use_self)]

use anyhow::{Context, Result};
use async_trait::async_trait;
use clap::Parser;
use config::{CacheEntries, CacheState};
use log::trace;

mod binnmu_buildinfo;
pub(crate) mod cli;
pub(crate) mod config;
mod grep_excuses;
mod nmu_eso;
mod nmu_list;
mod nmu_transition;
mod nmu_versionskew;
mod process_excuses;
pub(crate) mod source_packages;
pub(crate) mod udd_bugs;
pub(crate) mod utils;

use binnmu_buildinfo::BinNMUBuildinfo;
use cli::{DrtToolsCommands, DrtToolsOptions};
use grep_excuses::GrepExcuses;
use nmu_eso::NMUOutdatedBuiltUsing;
use nmu_list::NMUList;
use nmu_transition::NMUTransition;
use nmu_versionskew::NMUVersionSkew;
use process_excuses::ProcessExcuses;

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
            DrtToolsCommands::NMUList(nl_ots) => {
                Box::new(NMUList::new(&cache, &opts.base_options, nl_ots))
            }
        };
    execute_command(&cache, command.as_ref(), opts.base_options.force_processing).await
}
