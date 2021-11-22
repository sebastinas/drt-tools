// Copyright 2021 Sebastian Ramacher
// SPDX-License-Identifier: LGPL-3.0-or-later

//! # Helpers to generate commands for wanna-build
//!
//! This module provides builders to generate commands for [wanna-build](https://release.debian.org/wanna-build.txt).
//! It currently handles binNMUs only.

use crate::architectures::Architecture;
use std::fmt::{Display, Formatter};

/// A command to be executed by `wb`
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WBCommand(String);

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

impl<'a, T: ToString> WBCommandBuilder for T {
    fn build(&self) -> WBCommand {
        WBCommand(self.to_string())
    }
}

/// Architectures understood by `wb`
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WBArchitecture {
    /// The special `ANY` architecture, i.e., all architectures understood by wb except `all`
    Any,
    /// The special `ALL` architecture, i.e., all architectures understood by wb
    All,
    /// Specify an architecture
    Architecture(Architecture),
    /// Exclude a specific architecture
    NotArchitecture(Architecture),
}

impl Display for WBArchitecture {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            WBArchitecture::Any => write!(f, "ANY"),
            WBArchitecture::All => write!(f, "ALL"),
            WBArchitecture::Architecture(arch) => write!(f, "{}", arch),
            WBArchitecture::NotArchitecture(arch) => write!(f, "-{}", arch),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Base<'a> {
    source: &'a str,
    version: Option<&'a str>,
    architectures: Vec<WBArchitecture>,
    suite: Option<&'a str>,
}

impl<'a> Base<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            version: None,
            architectures: Vec::new(),
            suite: None,
        }
    }

    /// Specify version of the source package.
    fn with_version(&mut self, version: &'a str) -> &mut Self {
        self.version = Some(version);
        self
    }

    /// Specify suite. If not set, `unstable` is used.
    fn with_suite(&mut self, suite: &'a str) -> &mut Self {
        self.suite = Some(suite);
        self
    }

    /// Specify architectures. If not set, the `nmu` will be scheduled for `ANY`.
    fn with_architectures(&mut self, architectures: &[WBArchitecture]) -> &mut Self {
        self.architectures.extend_from_slice(architectures);
        self
    }
}

impl<'a> Display for Base<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.source)?;
        if let Some(version) = self.version {
            write!(f, "_{}", version)?;
        }
        write!(f, " . ")?;
        if self.architectures.is_empty() {
            write!(f, "{} ", WBArchitecture::Any)?;
        } else {
            for arch in &self.architectures {
                write!(f, "{} ", arch)?;
            }
        }
        write!(
            f,
            ". {}",
            if let Some(suite) = self.suite {
                suite
            } else {
                "unstable"
            },
        )
    }
}

/// Builder to create a `nmu` command
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BinNMU<'a> {
    base: Base<'a>,
    message: &'a str,
    nmu_version: Option<u32>,
    extra_depends: Option<&'a str>,
    priority: Option<i32>,
    dep_wait: Option<&'a str>,
}

impl<'a> BinNMU<'a> {
    /// Create a new `nmu` command for the given `source`.
    pub fn new(source: &'a str, message: &'a str) -> Self {
        Self {
            base: Base::new(source),
            message,
            nmu_version: None,
            extra_depends: None,
            priority: None,
            dep_wait: None,
        }
    }

    /// Specify version of the source package.
    pub fn with_version(&mut self, version: &'a str) -> &mut Self {
        self.base.with_version(version);
        self
    }

    /// Specify suite. If not set, `unstable` is used.
    pub fn with_suite(&mut self, suite: &'a str) -> &mut Self {
        self.base.with_suite(suite);
        self
    }

    /// Specify architectures. If not set, the `nmu` will be scheduled for `ANY`.
    pub fn with_architectures(&mut self, architectures: &[WBArchitecture]) -> &mut Self {
        self.base.with_architectures(architectures);
        self
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
        self.priority = Some(priority);
        self
    }

    /// Specify dependency-wait. If not set, no dependency-wait will be set.
    pub fn with_dependency_wait(&mut self, dw: &'a str) -> &mut Self {
        self.dep_wait = Some(dw);
        self
    }
}

impl<'a> Display for BinNMU<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "nmu ")?;
        if let Some(nmu_version) = self.nmu_version {
            write!(f, "{} ", nmu_version)?;
        }
        write!(f, "{} . -m \"{}\"", self.base, self.message)?;
        if let Some(extra_depends) = self.extra_depends {
            write!(f, " --extra-depends \"{}\"", extra_depends)?;
        }
        if let Some(dep_wait) = self.dep_wait {
            write!(
                f,
                "\n{}",
                DepWait {
                    base: self.base.clone(),
                    message: dep_wait
                }
            )?;
        }
        if let Some(priority) = self.priority {
            write!(
                f,
                "\n{}",
                BuildPriority {
                    base: self.base.clone(),
                    priority,
                }
            )?;
        }
        Ok(())
    }
}

/// Builder for the `dw` command
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DepWait<'a> {
    base: Base<'a>,
    message: &'a str,
}

impl<'a> DepWait<'a> {
    /// Create a new `dw` command for the given `source`.
    pub fn new(source: &'a str, message: &'a str) -> Self {
        Self {
            base: Base::new(source),
            message,
        }
    }

    /// Specify version of the source package.
    pub fn with_version(&mut self, version: &'a str) -> &mut Self {
        self.base.with_version(version);
        self
    }

    /// Specify suite. If not set, `unstable` is used.
    pub fn with_suite(&mut self, suite: &'a str) -> &mut Self {
        self.base.with_suite(suite);
        self
    }

    /// Specify architectures. If not set, the `nmu` will be scheduled for `ANY`.
    pub fn with_architectures(&mut self, architectures: &[WBArchitecture]) -> &mut Self {
        self.base.with_architectures(architectures);
        self
    }
}

impl<'a> Display for DepWait<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "dw {} . -m \"{}\"", self.base, self.message)
    }
}

/// Builder for the `bp` command
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildPriority<'a> {
    base: Base<'a>,
    priority: i32,
}

impl<'a> BuildPriority<'a> {
    /// Create a new `bp` command for the given `source`.
    pub fn new(source: &'a str, priority: i32) -> Self {
        Self {
            base: Base::new(source),
            priority,
        }
    }

    /// Specify version of the source package.
    pub fn with_version(&mut self, version: &'a str) -> &mut Self {
        self.base.with_version(version);
        self
    }

    /// Specify suite. If not set, `unstable` is used.
    pub fn with_suite(&mut self, suite: &'a str) -> &mut Self {
        self.base.with_suite(suite);
        self
    }

    /// Specify architectures. If not set, the `nmu` will be scheduled for `ANY`.
    pub fn with_architectures(&mut self, architectures: &[WBArchitecture]) -> &mut Self {
        self.base.with_architectures(architectures);
        self
    }
}

impl<'a> Display for BuildPriority<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "bp {} {}", self.priority, self.base)
    }
}

#[cfg(test)]
mod test {
    use crate::architectures::Architecture;
    use crate::wb::{BinNMU, BuildPriority, DepWait, WBArchitecture, WBCommandBuilder};

    #[test]
    fn binnmu() {
        assert_eq!(
            BinNMU::new("zathura", "Rebuild on buildd")
                .build()
                .to_string(),
            "nmu zathura . ANY . unstable . -m \"Rebuild on buildd\""
        );
        assert_eq!(
            BinNMU::new("zathura", "Rebuild on buildd")
                .with_nmu_version(3)
                .build()
                .to_string(),
            "nmu 3 zathura . ANY . unstable . -m \"Rebuild on buildd\""
        );
        assert_eq!(
            BinNMU::new("zathura", "Rebuild on buildd")
                .with_version("2.3.4")
                .build()
                .to_string(),
            "nmu zathura_2.3.4 . ANY . unstable . -m \"Rebuild on buildd\""
        );
        assert_eq!(
            BinNMU::new("zathura", "Rebuild on buildd")
                .with_architectures(&[
                    WBArchitecture::Any,
                    WBArchitecture::NotArchitecture(Architecture::I386)
                ])
                .build()
                .to_string(),
            "nmu zathura . ANY -i386 . unstable . -m \"Rebuild on buildd\""
        );
        assert_eq!(
            BinNMU::new("zathura", "Rebuild on buildd")
                .with_suite("testing")
                .build()
                .to_string(),
            "nmu zathura . ANY . testing . -m \"Rebuild on buildd\""
        );
        assert_eq!(
            BinNMU::new("zathura", "Rebuild on buildd")
                .with_extra_depends("libgirara-dev")
                .build()
                .to_string(),
            "nmu zathura . ANY . unstable . -m \"Rebuild on buildd\" --extra-depends \"libgirara-dev\""
        );
        assert_eq!(
            BinNMU::new("zathura", "Rebuild on buildd")
                .with_dependency_wait("libgirara-dev")
                .build()
                .to_string(),
            "nmu zathura . ANY . unstable . -m \"Rebuild on buildd\"\ndw zathura . ANY . unstable . -m \"libgirara-dev\""
        );
        assert_eq!(
            BinNMU::new("zathura", "Rebuild on buildd")
                .with_build_priority(-10)
                .build()
                .to_string(),
            "nmu zathura . ANY . unstable . -m \"Rebuild on buildd\"\nbp -10 zathura . ANY . unstable"
        );
    }

    #[test]
    fn nmu_builder() {
        let mut builder = BinNMU::new("zathura", "Rebuild on buildd");
        builder.with_version("1.2");
        assert_eq!(
            builder.build().to_string(),
            "nmu zathura_1.2 . ANY . unstable . -m \"Rebuild on buildd\""
        );
    }

    #[test]
    fn bp() {
        assert_eq!(
            BuildPriority::new("zathura", 10).build().to_string(),
            "bp 10 zathura . ANY . unstable"
        );
        assert_eq!(
            BuildPriority::new("zathura", 10)
                .with_version("2.3.4")
                .build()
                .to_string(),
            "bp 10 zathura_2.3.4 . ANY . unstable"
        );
        assert_eq!(
            BuildPriority::new("zathura", 10)
                .with_architectures(&[
                    WBArchitecture::Any,
                    WBArchitecture::NotArchitecture(Architecture::I386)
                ])
                .build()
                .to_string(),
            "bp 10 zathura . ANY -i386 . unstable"
        );
        assert_eq!(
            BuildPriority::new("zathura", 10)
                .with_suite("testing")
                .build()
                .to_string(),
            "bp 10 zathura . ANY . testing"
        );
    }

    #[test]
    fn dw() {
        assert_eq!(
            DepWait::new("zathura", "libgirara-dev").build().to_string(),
            "dw zathura . ANY . unstable . -m \"libgirara-dev\""
        );
        assert_eq!(
            DepWait::new("zathura", "libgirara-dev")
                .with_version("2.3.4")
                .build()
                .to_string(),
            "dw zathura_2.3.4 . ANY . unstable . -m \"libgirara-dev\""
        );
        assert_eq!(
            DepWait::new("zathura", "libgirara-dev")
                .with_architectures(&[
                    WBArchitecture::Any,
                    WBArchitecture::NotArchitecture(Architecture::I386)
                ])
                .build()
                .to_string(),
            "dw zathura . ANY -i386 . unstable . -m \"libgirara-dev\""
        );
        assert_eq!(
            DepWait::new("zathura", "libgirara-dev")
                .with_suite("testing")
                .build()
                .to_string(),
            "dw zathura . ANY . testing . -m \"libgirara-dev\""
        );
    }
}
