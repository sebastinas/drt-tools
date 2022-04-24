// Copyright 2022 Sebastian Ramacher
// SPDX-License-Identifier: LGPL-3.0-or-later

//! # Helpers to handle Debian archives
//!
//! These helpers includes enums to handle suites, codenames, and other fields found in Debian archive files.

use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::str::FromStr;

pub use crate::ParseError;

/// "Extensions" to a codename or a suite
///
/// This enum covers the archives for backports, security updates, (old)stable
/// updates and (old)stable proposed-updates.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Hash, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Extension {
    /// The backports extension
    Backports,
    /// The security extension
    Security,
    /// The updates extension
    Updates,
    /// The proposed-upates extension
    #[serde(rename = "proposed-updates")]
    ProposedUpdates,
}

impl Display for Extension {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Extension::Backports => "backports",
                Extension::Security => "security",
                Extension::Updates => "updates",
                Extension::ProposedUpdates => "proposed-updates",
            }
        )
    }
}

impl TryFrom<&str> for Extension {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "backports" => Ok(Extension::Backports),
            "security" => Ok(Extension::Security),
            "updates" => Ok(Extension::Updates),
            "proposed-updates" => Ok(Extension::ProposedUpdates),
            _ => Err(ParseError::InvalidExtension),
        }
    }
}

impl FromStr for Extension {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Extension::try_from(s)
    }
}

/// Debian archive suites
///
/// This enum describes the suite names found in the Debian archive.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Hash, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Suite {
    /// The unstable suite
    Unstable,
    /// The testing suite
    Testing(Option<Extension>),
    /// The stable suite
    Stable(Option<Extension>),
    /// The oldstable suite
    OldStable(Option<Extension>),
    /// The experimental suite
    Experimental,
}

impl Display for Suite {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        match self {
            Suite::Unstable => write!(f, "unstable"),
            Suite::Testing(None) => write!(f, "testing"),
            Suite::Stable(None) => write!(f, "stable"),
            Suite::OldStable(None) => write!(f, "oldstable"),
            Suite::Experimental => write!(f, "experimental"),
            Suite::Testing(Some(ext)) => write!(f, "testing-{}", ext),
            Suite::Stable(Some(ext)) => write!(f, "stable-{}", ext),
            Suite::OldStable(Some(ext)) => write!(f, "oldstable-{}", ext),
        }
    }
}

impl TryFrom<&str> for Suite {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "unstable" => Ok(Suite::Unstable),
            "testing" => Ok(Suite::Testing(None)),
            "stable" => Ok(Suite::Stable(None)),
            "oldstable" => Ok(Suite::OldStable(None)),
            "experimental" => Ok(Suite::Experimental),
            _ => {
                let s = value.split_once('-').ok_or(ParseError::InvalidSuite)?;
                let ext = Extension::try_from(s.1)?;
                match s.0 {
                    "testing" => Ok(Suite::Testing(Some(ext))),
                    "stable" => Ok(Suite::Stable(Some(ext))),
                    "oldstable" => Ok(Suite::OldStable(Some(ext))),
                    _ => Err(ParseError::InvalidSuite),
                }
            }
        }
    }
}

impl FromStr for Suite {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Suite::try_from(s)
    }
}

/// Debian archive codenames
///
/// This enum describes the codenames names found in the Debian archive.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Hash, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Codename {
    /// The unstable suite
    Sid,
    /// The testing suite
    Bookworm(Option<Extension>),
    /// The stable suite
    Bullseye(Option<Extension>),
    /// The oldstable suite
    Stretch(Option<Extension>),
    /// The experimental suite
    RCBuggy,
}

impl Display for Codename {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        match self {
            Codename::Sid => write!(f, "sid"),
            Codename::Bookworm(None) => write!(f, "bookworm"),
            Codename::Bullseye(None) => write!(f, "bullseye"),
            Codename::Stretch(None) => write!(f, "stretch"),
            Codename::RCBuggy => write!(f, "rc-buggy"),
            Codename::Bookworm(Some(ext)) => write!(f, "bookworm-{}", ext),
            Codename::Bullseye(Some(ext)) => write!(f, "bullseye-{}", ext),
            Codename::Stretch(Some(ext)) => write!(f, "stretch-{}", ext),
        }
    }
}

impl TryFrom<&str> for Codename {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "sid" => Ok(Codename::Sid),
            "bookworm" => Ok(Codename::Bookworm(None)),
            "bullseye" => Ok(Codename::Bullseye(None)),
            "stretch" => Ok(Codename::Stretch(None)),
            "rc-buggy" => Ok(Codename::RCBuggy),
            _ => {
                let s = value.split_once('-').ok_or(ParseError::InvalidCodename)?;
                let ext = Extension::try_from(s.1)?;
                match s.0 {
                    "bookworm" => Ok(Codename::Bookworm(Some(ext))),
                    "bullseye" => Ok(Codename::Bullseye(Some(ext))),
                    "stretch" => Ok(Codename::Stretch(Some(ext))),
                    _ => Err(ParseError::InvalidCodename),
                }
            }
        }
    }
}

impl FromStr for Codename {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Codename::try_from(s)
    }
}

impl From<Suite> for Codename {
    fn from(suite: Suite) -> Self {
        match suite {
            Suite::Unstable => Codename::Sid,
            Suite::Testing(ext) => Codename::Bookworm(ext),
            Suite::Stable(ext) => Codename::Bullseye(ext),
            Suite::OldStable(ext) => Codename::Stretch(ext),
            Suite::Experimental => Codename::RCBuggy,
        }
    }
}

impl From<Codename> for Suite {
    fn from(codename: Codename) -> Self {
        match codename {
            Codename::Sid => Suite::Unstable,
            Codename::Bookworm(ext) => Suite::Testing(ext),
            Codename::Bullseye(ext) => Suite::Stable(ext),
            Codename::Stretch(ext) => Suite::OldStable(ext),
            Codename::RCBuggy => Suite::Experimental,
        }
    }
}

/// Represents either a suite or codename
///
/// This enum is useful whenever a suite name or codename works
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Hash, Eq)]
pub enum SuiteOrCodename {
    /// A suite
    Suite(Suite),
    /// A codename
    Codename(Codename),
}

impl From<Codename> for SuiteOrCodename {
    fn from(codename: Codename) -> Self {
        SuiteOrCodename::Codename(codename)
    }
}

impl From<Suite> for SuiteOrCodename {
    fn from(suite: Suite) -> Self {
        SuiteOrCodename::Suite(suite)
    }
}

impl TryFrom<&str> for SuiteOrCodename {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match Suite::try_from(value) {
            Ok(suite) => Ok(SuiteOrCodename::Suite(suite)),
            Err(_) => match Codename::try_from(value) {
                Ok(codename) => Ok(SuiteOrCodename::Codename(codename)),
                Err(_) => Err(ParseError::InvalidSuiteOrCodename),
            },
        }
    }
}

impl FromStr for SuiteOrCodename {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        SuiteOrCodename::try_from(s)
    }
}

impl Display for SuiteOrCodename {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SuiteOrCodename::Suite(suite) => suite.fmt(f),
            SuiteOrCodename::Codename(codename) => codename.fmt(f),
        }
    }
}

impl From<SuiteOrCodename> for Suite {
    fn from(value: SuiteOrCodename) -> Self {
        match value {
            SuiteOrCodename::Suite(suite) => suite,
            SuiteOrCodename::Codename(codename) => Suite::from(codename),
        }
    }
}

impl From<SuiteOrCodename> for Codename {
    fn from(value: SuiteOrCodename) -> Self {
        match value {
            SuiteOrCodename::Suite(suite) => Codename::from(suite),
            SuiteOrCodename::Codename(codename) => codename,
        }
    }
}

/// Allowed values of the multi-arch field
#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MultiArch {
    /// MA: allowed
    Allowed,
    /// MA: foreign
    Foreign,
    /// MA: no
    No,
    /// MA: same
    Same,
}

impl Display for MultiArch {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            MultiArch::Allowed => write!(f, "allowed"),
            MultiArch::Foreign => write!(f, "foreign"),
            MultiArch::No => write!(f, "no"),
            MultiArch::Same => write!(f, "same"),
        }
    }
}

impl TryFrom<&str> for MultiArch {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "allowed" => Ok(MultiArch::Allowed),
            "foreign" => Ok(MultiArch::Foreign),
            "no" => Ok(MultiArch::No),
            "same" => Ok(MultiArch::Same),
            _ => Err(ParseError::InvalidMultiArch),
        }
    }
}

impl FromStr for MultiArch {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        MultiArch::try_from(s)
    }
}

#[cfg(test)]
mod test {
    use super::{Codename, Extension, Suite, SuiteOrCodename};

    #[test]
    fn suite_from_str() {
        assert_eq!(Suite::try_from("unstable").unwrap(), Suite::Unstable);
        assert_eq!(Suite::try_from("stable").unwrap(), Suite::Stable(None));
        assert_eq!(
            Suite::try_from("stable-backports").unwrap(),
            Suite::Stable(Some(Extension::Backports))
        );
    }

    #[test]
    fn codename_from_str() {
        assert_eq!(Codename::try_from("sid").unwrap(), Codename::Sid);
        assert_eq!(
            Codename::try_from("bullseye").unwrap(),
            Codename::Bullseye(None)
        );
        assert_eq!(
            Codename::try_from("bullseye-backports").unwrap(),
            Codename::Bullseye(Some(Extension::Backports))
        );
    }

    #[test]
    fn codename_from_suite() {
        assert_eq!(Codename::from(Suite::Unstable), Codename::Sid);
        assert_eq!(
            Codename::from(Suite::Stable(Some(Extension::Backports))),
            Codename::Bullseye(Some(Extension::Backports))
        );
    }

    #[test]
    fn suite_from_codename() {
        assert_eq!(Suite::from(Codename::Sid), Suite::Unstable);
        assert_eq!(
            Suite::from(Codename::Bullseye(Some(Extension::Backports))),
            Suite::Stable(Some(Extension::Backports))
        );
    }

    #[test]
    fn suite_or_codename_from_str() {
        assert_eq!(
            SuiteOrCodename::try_from("unstable").unwrap(),
            SuiteOrCodename::from(Suite::Unstable)
        );
        assert_eq!(
            SuiteOrCodename::try_from("sid").unwrap(),
            SuiteOrCodename::from(Codename::Sid)
        );
    }
}
