// Copyright 2022-2024 Sebastian Ramacher
// SPDX-License-Identifier: LGPL-3.0-or-later

//! # Helpers to handle Debian archives
//!
//! These helpers includes enums to handle suites, codenames, and other fields found in Debian archive files.

use std::fmt::{Display, Formatter};
use std::str::FromStr;

use serde::{Deserialize, Serialize, Serializer};

use crate::utils::TryFromStrVisitor;
pub use crate::ParseError;

/// "Extensions" to a codename or a suite
///
/// This enum covers the archives for backports, security updates, (old)stable
/// updates and (old)stable proposed-updates.
#[derive(Clone, Copy, Debug, PartialEq, Hash, Eq)]
pub enum Extension {
    /// The backports extension
    Backports,
    /// The security extension
    Security,
    /// The updates extension
    Updates,
    /// The proposed-upates extension
    ProposedUpdates,
}

impl AsRef<str> for Extension {
    fn as_ref(&self) -> &str {
        match self {
            Extension::Backports => "backports",
            Extension::Security => "security",
            Extension::Updates => "updates",
            Extension::ProposedUpdates => "proposed-updates",
        }
    }
}

impl Display for Extension {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_ref())
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
#[derive(Clone, Copy, Debug, PartialEq, Hash, Eq)]
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

impl Suite {
    /// Extend suite with an extension archive.
    ///
    /// An existing extension will overriden and the method has no effect for`unstable` and `experimental`.`
    pub fn with_extension(&self, extension: Extension) -> Self {
        match self {
            Suite::Unstable | Suite::Experimental => *self,
            Suite::Testing(_) => Suite::Testing(Some(extension)),
            Suite::Stable(_) => Suite::Stable(Some(extension)),
            Suite::OldStable(_) => Suite::OldStable(Some(extension)),
        }
    }

    /// Remove an extension archive from the suite.
    ///
    /// The method has no effect for`unstable` and `experimental`.`
    pub fn without_extension(&self) -> Self {
        match self {
            Suite::Unstable | Suite::Experimental => *self,
            Suite::Testing(_) => Suite::Testing(None),
            Suite::Stable(_) => Suite::Stable(None),
            Suite::OldStable(_) => Suite::OldStable(None),
        }
    }
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
            // The Release file from stable-proposed-updates calls the suite proposed-updaptes.
            "proposed-updates" => Ok(Suite::Stable(Some(Extension::ProposedUpdates))),
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

impl Serialize for Suite {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Suite {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(TryFromStrVisitor::<Suite>::new())
    }
}

/// Debian archive codenames
///
/// This enum describes the codenames names found in the Debian archive.
#[derive(Clone, Copy, Debug, PartialEq, Hash, Eq)]
pub enum Codename {
    /// The unstable suite
    Sid,
    /// The testing suite
    Trixie(Option<Extension>),
    /// The stable suite
    Bookworm(Option<Extension>),
    /// The oldstable suite
    Bullseye(Option<Extension>),
    /// The experimental suite
    RCBuggy,
}

impl Display for Codename {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        match self {
            Codename::Sid => write!(f, "sid"),
            Codename::Trixie(None) => write!(f, "trixie"),
            Codename::Bookworm(None) => write!(f, "bookworm"),
            Codename::Bullseye(None) => write!(f, "bullseye"),
            Codename::RCBuggy => write!(f, "rc-buggy"),
            Codename::Trixie(Some(ext)) => write!(f, "trixie-{}", ext),
            Codename::Bookworm(Some(ext)) => write!(f, "bookworm-{}", ext),
            Codename::Bullseye(Some(ext)) => write!(f, "bullseye-{}", ext),
        }
    }
}

impl TryFrom<&str> for Codename {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "sid" => Ok(Codename::Sid),
            "trixie" => Ok(Codename::Trixie(None)),
            "bookworm" => Ok(Codename::Bookworm(None)),
            "bullseye" => Ok(Codename::Bullseye(None)),
            "rc-buggy" => Ok(Codename::RCBuggy),
            _ => {
                let s = value.split_once('-').ok_or(ParseError::InvalidCodename)?;
                let ext = Extension::try_from(s.1)?;
                match s.0 {
                    "trixie" => Ok(Codename::Trixie(Some(ext))),
                    "bookworm" => Ok(Codename::Bookworm(Some(ext))),
                    "bullseye" => Ok(Codename::Bullseye(Some(ext))),
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
            Suite::Testing(ext) => Codename::Trixie(ext),
            Suite::Stable(ext) => Codename::Bookworm(ext),
            Suite::OldStable(ext) => Codename::Bullseye(ext),
            Suite::Experimental => Codename::RCBuggy,
        }
    }
}

impl From<Codename> for Suite {
    fn from(codename: Codename) -> Self {
        match codename {
            Codename::Sid => Suite::Unstable,
            Codename::Trixie(ext) => Suite::Testing(ext),
            Codename::Bookworm(ext) => Suite::Stable(ext),
            Codename::Bullseye(ext) => Suite::OldStable(ext),
            Codename::RCBuggy => Suite::Experimental,
        }
    }
}

impl Serialize for Codename {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Codename {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(TryFromStrVisitor::<Codename>::new())
    }
}

/// Represents either a suite or codename
///
/// This enum is useful whenever a suite name or codename works
#[derive(Clone, Copy, Debug, PartialEq, Hash, Eq)]
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

impl Serialize for SuiteOrCodename {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for SuiteOrCodename {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(TryFromStrVisitor::<SuiteOrCodename>::new())
    }
}

/// Allowed values of the multi-arch field
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
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

impl AsRef<str> for MultiArch {
    fn as_ref(&self) -> &str {
        match self {
            MultiArch::Allowed => "allowed",
            MultiArch::Foreign => "foreign",
            MultiArch::No => "no",
            MultiArch::Same => "same",
        }
    }
}

impl Display for MultiArch {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_ref())
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

/// Debian archive components
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Component {
    /// The `main` archive component
    Main,
    /// The `contrib` archive component
    Contrib,
    /// The `non-free` archive component
    #[serde(rename = "non-free")]
    NonFree,
    /// The `non-free-firmware` archive component
    #[serde(rename = "non-free-firmware")]
    NonFreeFirmware,
}

impl AsRef<str> for Component {
    fn as_ref(&self) -> &str {
        match self {
            Component::Main => "main",
            Component::Contrib => "contrib",
            Component::NonFree => "non-free",
            Component::NonFreeFirmware => "non-free-firmware",
        }
    }
}

impl Display for Component {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

impl TryFrom<&str> for Component {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "main" => Ok(Component::Main),
            "contrib" => Ok(Component::Contrib),
            "non-free" => Ok(Component::NonFree),
            "non-free-firmware" => Ok(Component::NonFreeFirmware),
            _ => Err(ParseError::InvalidComponent),
        }
    }
}

impl FromStr for Component {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Component::try_from(s)
    }
}

#[cfg(test)]
mod test {
    use super::*;

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
            Codename::Bookworm(Some(Extension::Backports))
        );
    }

    #[test]
    fn suite_from_codename() {
        assert_eq!(Suite::from(Codename::Sid), Suite::Unstable);
        assert_eq!(
            Suite::from(Codename::Bookworm(Some(Extension::Backports))),
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

    #[test]
    fn multi_arch_from_str() {
        assert_eq!(MultiArch::try_from("foreign").unwrap(), MultiArch::Foreign);
    }

    #[test]
    fn compoment_from_str() {
        assert_eq!(Component::try_from("main").unwrap(), Component::Main);
    }
}
