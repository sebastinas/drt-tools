// Copyright 2021 Sebastian Ramacher
// SPDX-License-Identifier: LGPL-3.0-or-later

//! # Helpers to generate commands for Debian's wanna-build service
//!
//! This module provides builders to generate commands for [wanna-build](https://release.debian.org/wanna-build.txt).

use std::fmt::{Display, Formatter};
use std::io::Write;
use std::process::{Command, Stdio};
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::architectures::{Architecture, ParseError};
use crate::archive::{Suite, SuiteOrCodename};
use crate::version::PackageVersion;

/// Errors when working with `wb`
#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid architecture {0} for wb command '{1}'")]
    /// An invalid architecture for a command was specified
    InvalidArchitecture(WBArchitecture, &'static str),
    #[error("unable to execute 'wb'")]
    /// Execution of `wb` failed
    ExecutionError,
    #[error("unable to exectue 'wb': {0}")]
    /// Execution of `wb` failed with IO error
    IOError(#[from] std::io::Error),
}

/// A command to be executed by `wb`
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub struct WBCommand(String);

impl WBCommand {
    /// Execute the command via `wb`
    ///
    /// This function runs `wb` and passes the commands on `stdin`.
    pub fn execute(&self) -> Result<(), Error> {
        let mut proc = Command::new("wb")
            .stdin(Stdio::piped())
            .spawn()
            .map_err(Error::from)?;
        if let Some(mut stdin) = proc.stdin.take() {
            stdin.write_all(self.0.as_bytes()).map_err(Error::from)?;
        } else {
            return Err(Error::ExecutionError);
        }
        proc.wait_with_output().map_err(Error::from)?;
        Ok(())
    }
}

impl Display for WBCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A trait to build `wb` commands
pub trait WBCommandBuilder {
    /// Build a `wb` command
    fn build(&self) -> WBCommand;
}

/// Architectures understood by `wb`
///
/// In addition to the the architectures from [Architecture], `wb` has two special "architectures"
/// named `ANY` (all binary-dependent architectures) and `ALL` (all architectures). Also, it
/// supports negation of architectures, e.g., `ANY -i386` refers to all binary-dependent
/// architectures without `i386`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum WBArchitecture {
    /// The special `ANY` architecture, i.e., all architectures understood by wb except `all`
    Any,
    /// The special `ALL` architecture, i.e., all architectures understood by wb
    All,
    /// Specify an architecture
    Architecture(Architecture),
    /// Exclude a specific architecture
    ExcludeArchitecture(Architecture),
}

impl Display for WBArchitecture {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Any => write!(f, "ANY"),
            Self::All => write!(f, "ALL"),
            Self::Architecture(arch) => write!(f, "{arch}"),
            Self::ExcludeArchitecture(arch) => write!(f, "-{arch}"),
        }
    }
}

impl TryFrom<&str> for WBArchitecture {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "ANY" => Ok(Self::Any),
            "ALL" => Ok(Self::All),
            _ => {
                if let Some(stripped) = value.strip_prefix('-') {
                    Ok(Self::ExcludeArchitecture(stripped.try_into()?))
                } else {
                    Ok(Self::Architecture(value.try_into()?))
                }
            }
        }
    }
}

impl FromStr for WBArchitecture {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s)
    }
}

/// Specifier for a source with version, architecture and suite
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceSpecifier<'a> {
    source: &'a str,
    version: Option<&'a PackageVersion>,
    architectures: Vec<WBArchitecture>,
    suite: Option<SuiteOrCodename>,
}

impl<'a> SourceSpecifier<'a> {
    /// Create a new source specifier for the given source package name.
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            version: None,
            architectures: Vec::new(),
            suite: None,
        }
    }

    /// Specify version of the source package.
    pub fn with_version(&mut self, version: &'a PackageVersion) -> &mut Self {
        self.version = Some(version);
        self
    }

    /// Specify suite. If not set, `unstable` is used.
    pub fn with_suite(&mut self, suite: SuiteOrCodename) -> &mut Self {
        self.suite = Some(suite);
        self
    }

    /// Specify architectures. If not set, the `nmu` will be scheduled for `ANY`.
    pub fn with_architectures(&mut self, architectures: &[WBArchitecture]) -> &mut Self {
        self.architectures.extend_from_slice(architectures);
        self
    }

    /// Specify architectures. If not set, the `nmu` will be scheduled for `ANY`.
    pub fn with_archive_architectures(&mut self, architectures: &[Architecture]) -> &mut Self {
        self.architectures.extend(
            architectures
                .iter()
                .copied()
                .map(WBArchitecture::Architecture),
        );
        self
    }
}

impl Display for SourceSpecifier<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.source)?;
        if let Some(version) = self.version {
            write!(f, "_{version}")?;
        }
        write!(f, " . ")?;
        if self.architectures.is_empty() {
            write!(f, "{} ", WBArchitecture::Any)?;
        } else {
            for arch in &self.architectures {
                write!(f, "{arch} ")?;
            }
        }
        write!(f, ". {}", self.suite.unwrap_or(Suite::Unstable.into()))
    }
}

/// Builder to create a `nmu` command
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BinNMU<'a> {
    source: &'a SourceSpecifier<'a>,
    message: &'a str,
    nmu_version: Option<u32>,
    extra_depends: Option<&'a str>,
    priority: Option<i32>,
    dep_wait: Option<&'a str>,
}

impl<'a> BinNMU<'a> {
    /// Create a new `nmu` command for the given `source`.
    pub fn new(source: &'a SourceSpecifier<'a>, message: &'a str) -> Result<Self, Error> {
        for arch in &source.architectures {
            match arch {
                // unable to nmu with source, -source, ALL, all
                WBArchitecture::Architecture(Architecture::Source | Architecture::All)
                | WBArchitecture::ExcludeArchitecture(Architecture::Source | Architecture::All)
                | WBArchitecture::All => {
                    return Err(Error::InvalidArchitecture(*arch, "nmu"));
                }
                _ => {}
            }
        }
        Ok(Self {
            source,
            message,
            nmu_version: None,
            extra_depends: None,
            priority: None,
            dep_wait: None,
        })
    }

    /// Specify the binNMU version. If not set, `wb` tries to auto-detect the binNMU version.
    pub fn with_nmu_version(&mut self, version: u32) -> &mut Self {
        self.nmu_version = Some(version);
        self
    }

    /// Specify extra dependencies.
    pub fn with_extra_depends(&mut self, extra_depends: &'a str) -> &mut Self {
        self.extra_depends = Some(extra_depends);
        self
    }

    /// Specify build priority. If not set, the build priority will not be changed.
    pub fn with_build_priority(&mut self, priority: i32) -> &mut Self {
        if priority != 0 {
            self.priority = Some(priority);
        } else {
            self.priority = None;
        }
        self
    }

    /// Specify dependency-wait. If not set, no dependency-wait will be set.
    pub fn with_dependency_wait(&mut self, dw: &'a str) -> &mut Self {
        self.dep_wait = Some(dw);
        self
    }
}

impl Display for BinNMU<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "nmu ")?;
        if let Some(nmu_version) = self.nmu_version {
            write!(f, "{nmu_version} ")?;
        }
        write!(f, "{} . -m \"{}\"", self.source, self.message)?;
        if let Some(extra_depends) = self.extra_depends {
            write!(f, " --extra-depends \"{extra_depends}\"")?;
        }
        if let Some(dep_wait) = self.dep_wait {
            write!(
                f,
                "\n{}",
                DepWait {
                    source: self.source,
                    message: dep_wait
                }
            )?;
        }
        if let Some(priority) = self.priority {
            write!(
                f,
                "\n{}",
                BuildPriority {
                    source: self.source,
                    priority,
                }
            )?;
        }
        Ok(())
    }
}

impl WBCommandBuilder for BinNMU<'_> {
    fn build(&self) -> WBCommand {
        WBCommand(self.to_string())
    }
}

/// Builder for the `dw` command
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DepWait<'a> {
    source: &'a SourceSpecifier<'a>,
    message: &'a str,
}

impl<'a> DepWait<'a> {
    /// Create a new `dw` command for the given `source`.
    pub fn new(source: &'a SourceSpecifier<'a>, message: &'a str) -> Result<Self, Error> {
        for arch in &source.architectures {
            match arch {
                // unable to dw with source, -source
                WBArchitecture::Architecture(Architecture::Source)
                | WBArchitecture::ExcludeArchitecture(Architecture::Source) => {
                    return Err(Error::InvalidArchitecture(*arch, "dw"));
                }
                _ => {}
            }
        }

        Ok(Self { source, message })
    }
}

impl Display for DepWait<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "dw {} . -m \"{}\"", self.source, self.message)
    }
}

impl WBCommandBuilder for DepWait<'_> {
    fn build(&self) -> WBCommand {
        WBCommand(self.to_string())
    }
}

/// Builder for the `bp` command
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildPriority<'a> {
    source: &'a SourceSpecifier<'a>,
    priority: i32,
}

impl<'a> BuildPriority<'a> {
    /// Create a new `bp` command for the given `source`.
    pub fn new(source: &'a SourceSpecifier<'a>, priority: i32) -> Result<Self, Error> {
        for arch in &source.architectures {
            match *arch {
                // unable to bp with source, -source
                WBArchitecture::Architecture(Architecture::Source)
                | WBArchitecture::ExcludeArchitecture(Architecture::Source) => {
                    return Err(Error::InvalidArchitecture(*arch, "bp"));
                }
                _ => {}
            }
        }

        Ok(Self { source, priority })
    }
}

impl Display for BuildPriority<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "bp {} {}", self.priority, self.source)
    }
}

impl WBCommandBuilder for BuildPriority<'_> {
    fn build(&self) -> WBCommand {
        WBCommand(self.to_string())
    }
}

/// Builder for the `fail` command
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fail<'a> {
    source: &'a SourceSpecifier<'a>,
    message: &'a str,
}

impl<'a> Fail<'a> {
    /// Create a new `fail` command for the given `source`.
    pub fn new(source: &'a SourceSpecifier<'a>, message: &'a str) -> Result<Self, Error> {
        for arch in &source.architectures {
            match *arch {
                // unable to fail with source, -source
                WBArchitecture::Architecture(Architecture::Source)
                | WBArchitecture::ExcludeArchitecture(Architecture::Source) => {
                    return Err(Error::InvalidArchitecture(*arch, "fail"));
                }
                _ => {}
            }
        }

        Ok(Self { source, message })
    }
}

impl Display for Fail<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "fail {} . -m \"{}\"", self.source, self.message)
    }
}

impl WBCommandBuilder for Fail<'_> {
    fn build(&self) -> WBCommand {
        WBCommand(self.to_string())
    }
}

#[cfg(test)]
mod test {
    use super::{
        BinNMU, BuildPriority, DepWait, Fail, SourceSpecifier, WBArchitecture, WBCommandBuilder,
    };
    use crate::architectures::Architecture;
    use crate::archive::{Suite, SuiteOrCodename};

    const TESTING: SuiteOrCodename = SuiteOrCodename::Suite(Suite::Testing(None));

    #[test]
    fn arch_from_str() {
        assert_eq!(
            WBArchitecture::try_from("ANY").unwrap(),
            WBArchitecture::Any
        );
        assert_eq!(
            WBArchitecture::try_from("ALL").unwrap(),
            WBArchitecture::All
        );
        assert_eq!(
            WBArchitecture::try_from("amd64").unwrap(),
            WBArchitecture::Architecture(Architecture::Amd64)
        );
        assert_eq!(
            WBArchitecture::try_from("-amd64").unwrap(),
            WBArchitecture::ExcludeArchitecture(Architecture::Amd64)
        );
        assert!(WBArchitecture::try_from("-ALL").is_err());
    }

    #[test]
    fn binnmu() {
        assert_eq!(
            BinNMU::new(&SourceSpecifier::new("zathura"), "Rebuild on buildd")
                .unwrap()
                .build()
                .to_string(),
            "nmu zathura . ANY . unstable . -m \"Rebuild on buildd\""
        );
        assert_eq!(
            BinNMU::new(&SourceSpecifier::new("zathura"), "Rebuild on buildd")
                .unwrap()
                .with_nmu_version(3)
                .build()
                .to_string(),
            "nmu 3 zathura . ANY . unstable . -m \"Rebuild on buildd\""
        );
        assert_eq!(
            BinNMU::new(
                SourceSpecifier::new("zathura").with_version(&"2.3.4".try_into().unwrap()),
                "Rebuild on buildd"
            )
            .unwrap()
            .build()
            .to_string(),
            "nmu zathura_2.3.4 . ANY . unstable . -m \"Rebuild on buildd\""
        );
        assert_eq!(
            BinNMU::new(
                SourceSpecifier::new("zathura").with_architectures(&[
                    WBArchitecture::Any,
                    WBArchitecture::ExcludeArchitecture(Architecture::I386)
                ]),
                "Rebuild on buildd"
            )
            .unwrap()
            .build()
            .to_string(),
            "nmu zathura . ANY -i386 . unstable . -m \"Rebuild on buildd\""
        );
        assert_eq!(
            BinNMU::new(
                SourceSpecifier::new("zathura").with_suite(TESTING),
                "Rebuild on buildd"
            )
            .unwrap()
            .build()
            .to_string(),
            "nmu zathura . ANY . testing . -m \"Rebuild on buildd\""
        );
        assert_eq!(
            BinNMU::new(&SourceSpecifier::new("zathura"), "Rebuild on buildd").unwrap()
                .with_extra_depends("libgirara-dev")
                .build()
                .to_string(),
            "nmu zathura . ANY . unstable . -m \"Rebuild on buildd\" --extra-depends \"libgirara-dev\""
        );
        assert_eq!(
            BinNMU::new(&SourceSpecifier::new("zathura"), "Rebuild on buildd").unwrap()
                .with_dependency_wait("libgirara-dev")
                .build()
                .to_string(),
            "nmu zathura . ANY . unstable . -m \"Rebuild on buildd\"\ndw zathura . ANY . unstable . -m \"libgirara-dev\""
        );
        assert_eq!(
            BinNMU::new(&SourceSpecifier::new("zathura"), "Rebuild on buildd").unwrap()
                .with_build_priority(-10)
                .build()
                .to_string(),
            "nmu zathura . ANY . unstable . -m \"Rebuild on buildd\"\nbp -10 zathura . ANY . unstable"
        );
    }

    #[test]
    fn nmu_builder() {
        let source = SourceSpecifier::new("zathura");
        let mut builder = BinNMU::new(&source, "Rebuild on buildd").unwrap();
        builder.with_nmu_version(3);
        assert_eq!(
            builder.build().to_string(),
            "nmu 3 zathura . ANY . unstable . -m \"Rebuild on buildd\""
        );

        builder.with_build_priority(0);
        assert_eq!(
            builder.build().to_string(),
            "nmu 3 zathura . ANY . unstable . -m \"Rebuild on buildd\""
        );
    }

    #[test]
    fn bp() {
        assert_eq!(
            BuildPriority::new(&SourceSpecifier::new("zathura"), 10)
                .unwrap()
                .build()
                .to_string(),
            "bp 10 zathura . ANY . unstable"
        );
        assert_eq!(
            BuildPriority::new(
                SourceSpecifier::new("zathura").with_version(&"2.3.4".try_into().unwrap()),
                10
            )
            .unwrap()
            .build()
            .to_string(),
            "bp 10 zathura_2.3.4 . ANY . unstable"
        );
        assert_eq!(
            BuildPriority::new(
                SourceSpecifier::new("zathura").with_architectures(&[
                    WBArchitecture::Any,
                    WBArchitecture::ExcludeArchitecture(Architecture::I386)
                ]),
                10
            )
            .unwrap()
            .build()
            .to_string(),
            "bp 10 zathura . ANY -i386 . unstable"
        );
        assert_eq!(
            BuildPriority::new(SourceSpecifier::new("zathura").with_suite(TESTING), 10)
                .unwrap()
                .build()
                .to_string(),
            "bp 10 zathura . ANY . testing"
        );
    }

    #[test]
    fn dw() {
        assert_eq!(
            DepWait::new(&SourceSpecifier::new("zathura"), "libgirara-dev")
                .unwrap()
                .build()
                .to_string(),
            "dw zathura . ANY . unstable . -m \"libgirara-dev\""
        );
        assert_eq!(
            DepWait::new(
                SourceSpecifier::new("zathura").with_version(&"2.3.4".try_into().unwrap()),
                "libgirara-dev"
            )
            .unwrap()
            .build()
            .to_string(),
            "dw zathura_2.3.4 . ANY . unstable . -m \"libgirara-dev\""
        );
        assert_eq!(
            DepWait::new(
                SourceSpecifier::new("zathura").with_architectures(&[
                    WBArchitecture::Any,
                    WBArchitecture::ExcludeArchitecture(Architecture::I386)
                ]),
                "libgirara-dev"
            )
            .unwrap()
            .build()
            .to_string(),
            "dw zathura . ANY -i386 . unstable . -m \"libgirara-dev\""
        );
        assert_eq!(
            DepWait::new(
                SourceSpecifier::new("zathura").with_suite(TESTING),
                "libgirara-dev"
            )
            .unwrap()
            .build()
            .to_string(),
            "dw zathura . ANY . testing . -m \"libgirara-dev\""
        );
    }

    #[test]
    fn fail() {
        assert_eq!(
            Fail::new(&SourceSpecifier::new("zathura"), "#1234")
                .unwrap()
                .build()
                .to_string(),
            "fail zathura . ANY . unstable . -m \"#1234\""
        );
    }
}
