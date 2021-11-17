// Copyright 2021 Sebastian Ramacher
// SPDX-License-Identifier: LGPL-3.0-or-later

use crate::architectures::Architecture;
use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WBCommand(String);

impl Display for WBCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub trait WBCommandBuilder {
    fn build(&self) -> WBCommand;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BinNMUArchitecture {
    Any,
    Architecture(Architecture),
    SkipArchitecture(Architecture),
}

impl Display for BinNMUArchitecture {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            BinNMUArchitecture::Any => write!(f, "ANY"),
            BinNMUArchitecture::Architecture(arch) => write!(f, "{}", arch),
            BinNMUArchitecture::SkipArchitecture(arch) => write!(f, "-{}", arch),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BinNMU<'a> {
    source: &'a str,
    version: Option<&'a str>,
    nmu_version: Option<u32>,
    architectures: Vec<BinNMUArchitecture>,
    message: &'a str,
    suite: Option<&'a str>,
    extra_depends: Option<&'a str>,
}

impl<'a> BinNMU<'a> {
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

    pub fn with_version(&mut self, version: &'a str) -> &mut Self {
        self.version = Some(version);
        self
    }

    pub fn with_nmu_version(&mut self, version: u32) -> &mut Self {
        self.nmu_version = Some(version);
        self
    }

    pub fn with_suite(&mut self, suite: &'a str) -> &mut Self {
        self.suite = Some(suite);
        self
    }

    pub fn with_extra_depends(&mut self, extra_depends: &'a str) -> &mut Self {
        self.extra_depends = Some(extra_depends);
        self
    }

    pub fn with_architectures(&mut self, architectures: &[BinNMUArchitecture]) -> &mut Self {
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
        if let Some(version) = &self.version {
            write!(f, "_{}", version)?;
        }
        write!(f, " . ")?;
        for arch in &self.architectures {
            write!(f, "{} ", arch)?;
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct DepWait {
    source: String,
    version: Option<String>,
    architectures: Vec<BinNMUArchitecture>,
    message: String,
}
