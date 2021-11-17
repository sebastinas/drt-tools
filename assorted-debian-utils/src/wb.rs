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

/// Builder to create a `nmu` command
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BinNMU<'a> {
    source: &'a str,
    version: Option<&'a str>,
    nmu_version: Option<u32>,
    architectures: Vec<WBArchitecture>,
    message: &'a str,
    suite: Option<&'a str>,
    extra_depends: Option<&'a str>,
}

impl<'a> BinNMU<'a> {
    /// Create a new `nmu` command for the given `source`.
    pub fn new(source: &'a str, message: &'a str) -> Self {
        Self {
            source,
            version: None,
            nmu_version: None,
            architectures: Vec::new(),
            message,
            suite: None,
            extra_depends: None,
        }
    }

    /// Specify version of the source package.
    pub fn with_version(&mut self, version: &'a str) -> &mut Self {
        self.version = Some(version);
        self
    }

    /// Specify the binNMU version. If not set, `wb` tries to auto-detect the binNMU version.
    pub fn with_nmu_version(&mut self, version: u32) -> &mut Self {
        self.nmu_version = Some(version);
        self
    }

    /// Specify suite. If not set, `unstable` is used.
    pub fn with_suite(&mut self, suite: &'a str) -> &mut Self {
        self.suite = Some(suite);
        self
    }

    /// Specify extra dependencies.
    pub fn with_extra_depends(&mut self, extra_depends: &'a str) -> &mut Self {
        self.extra_depends = Some(extra_depends);
        self
    }

    /// Specify architectures. If not set, the `nmu` will be scheduled for `ANY`.
    pub fn with_architectures(&mut self, architectures: &[WBArchitecture]) -> &mut Self {
        self.architectures.extend_from_slice(architectures);
        self
    }
}

impl<'a> WBCommandBuilder for BinNMU<'a> {
    fn build(&self) -> WBCommand {
        WBCommand(self.to_string())
    }
}

impl<'a> Display for BinNMU<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "nmu ")?;
        if let Some(nmu_version) = self.nmu_version {
            write!(f, "{} ", nmu_version)?;
        }
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
            ". {} . -m \"{}\"",
            if let Some(suite) = &self.suite {
                &suite
            } else {
                "unstable"
            },
            self.message
        )?;
        if let Some(extra_depends) = &self.extra_depends {
            write!(f, " --extra-depends \"{}\"", extra_depends)?;
        }
        Ok(())
    }
}

/*
#[derive(Clone, Debug, Eq, PartialEq)]
struct DepWait {
    source: String,
    version: Option<String>,
    architectures: Vec<WBArchitecture>,
    message: String,
}
*/

#[cfg(test)]
mod test {
    use crate::architectures::Architecture;
    use crate::wb::{BinNMU, WBArchitecture, WBCommandBuilder};

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
}
