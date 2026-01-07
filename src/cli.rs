// Copyright 2021-2025 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{fmt, path::PathBuf, str::FromStr};

use assorted_debian_utils::{
    archive::SuiteOrCodename, package::PackageRelationship, wb::WBArchitecture,
};
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
pub struct BaseOptions {
    /// Force download of files
    ///
    /// If this option is set, all files will be fetched from mirrors, UDD, etc.
    #[clap(long)]
    pub force_download: bool,
    /// Force processing.
    #[clap(short, long = "force")]
    pub force_processing: bool,
    /// Only print actions to perform without running any commands
    ///
    /// This option is especially useful if one wants to produce a list of `wb`
    /// commands where the output is copied to the `buildd` server and directly
    /// executed there.
    #[clap(short = 'n')]
    pub dry_run: bool,
    #[clap(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity,
    /// Archive mirror
    ///
    /// Information on packages, sources and releases is downloaded from this mirror.
    #[clap(long, default_value = "https://deb.debian.org/debian")]
    pub mirror: String,
    /// `buildd` server
    ///
    /// To schedule `wanna-build` actions, a SSH connection is established to this server.
    #[clap(long, default_value = "wuiet.debian.org")]
    pub buildd: String,
}

#[derive(Debug, Parser)]
pub struct BinNMUsOptions {
    /// Message for binNMUs
    #[clap(short, long)]
    pub message: String,
    /// Build priority
    ///
    /// If specified, the binNMUs are scheduled with the given build priority.
    /// Builds with a positive priority will be built earlier.
    #[clap(long = "bp")]
    pub build_priority: Option<i32>,
    /// Dependency-wait
    ///
    /// If specified, the builds will wait until the given dependency relation is satisfied.
    #[clap(long = "dw")]
    pub dep_wait: Option<Vec<PackageRelationship>>,
    /// Extra dependencies
    ///
    /// If specified, the given dependency will be installed during the build.
    #[clap(long)]
    pub extra_depends: Option<Vec<PackageRelationship>>,
    /// Suite for binNMUs.
    #[clap(short, long, default_value = "unstable")]
    pub suite: SuiteOrCodename,
    /// Architectures for binNMUs
    ///
    /// If no architectures are specified, the binNMUs are scheduled with ANY.
    /// Otherwise, the architectures specified here are taken. The option
    /// supports the special architecture `ANY` as well as removing
    /// architectures from `ANY` by specifying `-$arch`.
    #[clap(short, long)]
    pub architecture: Option<Vec<WBArchitecture>>,
}

#[derive(Debug, Parser)]
pub struct GrepExcusesOptions {
    /// Currently not implemented
    ///
    /// This is currently only provided as option for compatibility with `grep-excuses` from `devscripts`.
    #[clap(long)]
    pub autopkgtests: bool,
    /// The maintainer or package to grep for
    #[clap(num_args = 1, required = true)]
    pub maintainer_package: Vec<String>,
}

#[derive(Debug, Parser)]
pub struct BinNMUBuildinfoOptions {
    #[clap(flatten)]
    pub binnmu_options: BinNMUsOptions,
    /// Input files
    pub inputs: Vec<PathBuf>,
}

#[derive(Debug, Parser)]
pub struct ProcessExcusesOptions {
    /// Ignore age of packages
    #[clap(long)]
    pub ignore_age: bool,
    /// Ignore results from autopkgtests
    #[clap(long)]
    pub ignore_autopkgtests: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(clippy::enum_variant_names)]
pub enum Field {
    BuiltUsing,
    StaticBuiltUsing,
    XCargoBuiltUsing,
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BuiltUsing => write!(f, "Built-Using"),
            Self::StaticBuiltUsing => write!(f, "Static-Built-Using"),
            Self::XCargoBuiltUsing => write!(f, "X-Cargo-Built-Using"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub struct ParseError;

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid reference field")
    }
}

impl FromStr for Field {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Built-Using" => Ok(Self::BuiltUsing),
            "Static-Built-Using" => Ok(Self::StaticBuiltUsing),
            "X-Cargo-Built-Using" => Ok(Self::XCargoBuiltUsing),
            _ => Err(ParseError),
        }
    }
}

#[derive(Debug, Parser)]
pub struct NMUOutdatedBuiltUsingOptions {
    /// Build priority
    ///
    /// If specified, the binNMUs are scheduled with the given build priority.
    /// Builds with a positive priority will be built earlier.
    #[clap(long = "bp", default_value_t = -50)]
    pub build_priority: i32,
    /// Suite for binNMUs
    #[clap(short, long, default_value_t = SuiteOrCodename::UNSTABLE)]
    pub suite: SuiteOrCodename,
    /// Select the binary package field to check for outdated information
    ///
    /// By default, the `Built-Using` field is checked. Other supported values
    /// are `Static-Built-Using` and `X-Cargo-Built-Using`. This option can be
    /// specfied multiple times to check several fields at the same time.
    #[clap(long)]
    pub field: Vec<Field>,
}

#[derive(Debug, Parser)]
pub struct NMUVersionSkewOptions {
    /// Build priority
    ///
    /// If specified, the binNMUs are scheduled with the given build priority.
    /// Builds with a positive priority will be built earlier.
    #[clap(long = "bp", default_value_t = -50)]
    pub build_priority: i32,
    /// Suite for binNMUs.
    #[clap(short, long, default_value_t = SuiteOrCodename::UNSTABLE)]
    pub suite: SuiteOrCodename,
}

#[derive(Debug, Parser)]
pub struct NMUListOptions {
    #[clap(flatten)]
    pub binnmu_options: BinNMUsOptions,
    /// Input file with a list of packages
    ///
    /// If not specified, the list of packages will be read from the standard input.
    pub input: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
pub enum DrtToolsCommands {
    /// Process current excuses.yaml and prepare a list of binNMUs required for
    /// testing migration and list of unblocks
    ///
    /// For unblocks, this command parses the current excuses and prepares a
    /// list of packages in testing-proposed-updates and packages that have been
    /// rebuilt in unstable but are blocked by the freeze.
    ///
    /// For rebuilds, the command checks for packages that could migrate to
    /// testing if all binaries would have been built on a `buildd`. Packages
    /// are only consider if all other checks pass (e.g., `piuparts` and
    /// `autopkgtests`) that also have reached half of the required age are
    /// considered.
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
    NMUTransition(NMUListOptions),
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
    /// Collect a list of all packages that refer to `Extra-Source-Only: yes`
    /// source packages in their `Built-Using` field. The command also support
    /// to check packages where `Static-Built-Using` or `X-Cargo-Built-Using`
    /// refers to packages no longer in the archive. The latter is useful to
    /// check for rebuilds of Rust and Go packages.
    #[clap(name = "nmu-eso")]
    NMUOutdatedBuiltUsing(NMUOutdatedBuiltUsingOptions),
    /// Prepare rebuilds for version skew in Multi-Arch: same packages
    ///
    /// Packages producing MA: same binary packages are required to have the
    /// same binNMU version number across all architectures. This command checks
    /// the archive for packages where this constraint is currently not met and
    /// produces a list of binNMUs to sync the binNMU versions.
    #[clap(name = "nmu-version-skew")]
    NMUVersionSkew(NMUVersionSkewOptions),
    /// BinNMU a list of packages
    ///
    /// This command reads a list of source packages from stdin or a given file
    /// and takes it as input to produce/schedule commands to binNMU those
    /// packages.  The command is able to handle both version-less lists and
    /// versioned source packages formatted as `source_version`.
    ///
    /// If an architecture is specified, but the package to be rebuilt produces
    /// a `Multi-Arch: same` binary, a binNMU for `ANY` will be scheduled
    /// instead.
    #[clap(name = "nmu-list")]
    NMUList(NMUListOptions),
}

#[derive(Debug, Parser)]
pub struct DrtToolsOptions {
    #[clap(flatten)]
    pub base_options: BaseOptions,
    #[clap(subcommand)]
    pub command: DrtToolsCommands,
}
