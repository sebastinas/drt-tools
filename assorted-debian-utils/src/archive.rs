// Copyright 2022-2024 Sebastian Ramacher
// SPDX-License-Identifier: LGPL-3.0-or-later

//! # Helpers to handle Debian archives
//!
//! These helpers includes enums to handle suites, codenames, and other fields found in Debian archive files.

use std::{
    fmt::{Display, Formatter},
    hash::{Hash, Hasher},
    str::FromStr,
};

use serde::{Deserialize, Serialize, Serializer};

pub use crate::ParseError;
use crate::utils::TryFromStrVisitor;

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
            Self::Backports => "backports",
            Self::Security => "security",
            Self::Updates => "updates",
            Self::ProposedUpdates => "proposed-updates",
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
            "backports" => Ok(Self::Backports),
            "security" => Ok(Self::Security),
            "updates" => Ok(Self::Updates),
            "proposed-updates" => Ok(Self::ProposedUpdates),
            _ => Err(ParseError::InvalidExtension),
        }
    }
}

impl FromStr for Extension {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s)
    }
}

/// Trait to add/remove archive extensions from `Suite` and `Codename`
pub trait WithExtension {
    /// Extend suite with an extension archive.
    ///
    /// An existing extension will overriden and the method has no effect for`unstable` and `experimental`.
    fn with_extension(&self, extension: Extension) -> Self;

    /// Remove an extension archive from the suite.
    ///
    /// The method has no effect for`unstable` and `experimental`.
    fn without_extension(&self) -> Self;
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

impl WithExtension for Suite {
    fn with_extension(&self, extension: Extension) -> Self {
        match self {
            Self::Unstable | Self::Experimental => *self,
            Self::Testing(_) => Self::Testing(Some(extension)),
            Self::Stable(_) => Self::Stable(Some(extension)),
            Self::OldStable(_) => Self::OldStable(Some(extension)),
        }
    }

    fn without_extension(&self) -> Self {
        match self {
            Self::Unstable | Self::Experimental => *self,
            Self::Testing(_) => Self::Testing(None),
            Self::Stable(_) => Self::Stable(None),
            Self::OldStable(_) => Self::OldStable(None),
        }
    }
}

impl Display for Suite {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        match self {
            Self::Unstable => write!(f, "unstable"),
            Self::Testing(None) => write!(f, "testing"),
            Self::Stable(None) => write!(f, "stable"),
            Self::OldStable(None) => write!(f, "oldstable"),
            Self::Experimental => write!(f, "experimental"),
            Self::Testing(Some(ext)) => write!(f, "testing-{ext}"),
            Self::Stable(Some(ext)) => write!(f, "stable-{ext}"),
            Self::OldStable(Some(ext)) => write!(f, "oldstable-{ext}"),
        }
    }
}

impl TryFrom<&str> for Suite {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "unstable" => Ok(Self::Unstable),
            "testing" => Ok(Self::Testing(None)),
            "stable" => Ok(Self::Stable(None)),
            "oldstable" => Ok(Self::OldStable(None)),
            // The Release file from stable-proposed-updates calls the suite proposed-updaptes.
            "proposed-updates" => Ok(Self::Stable(Some(Extension::ProposedUpdates))),
            "experimental" => Ok(Self::Experimental),
            _ => {
                let s = value.split_once('-').ok_or(ParseError::InvalidSuite)?;
                let ext = Extension::try_from(s.1)?;
                match s.0 {
                    "testing" => Ok(Self::Testing(Some(ext))),
                    "stable" => Ok(Self::Stable(Some(ext))),
                    "oldstable" => Ok(Self::OldStable(Some(ext))),
                    _ => Err(ParseError::InvalidSuite),
                }
            }
        }
    }
}

impl FromStr for Suite {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s)
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
        deserializer.deserialize_str(TryFromStrVisitor::<Self>::new("a suite name"))
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

impl WithExtension for Codename {
    fn with_extension(&self, extension: Extension) -> Self {
        match self {
            Self::Sid | Self::RCBuggy => *self,
            Self::Trixie(_) => Self::Trixie(Some(extension)),
            Self::Bookworm(_) => Self::Bookworm(Some(extension)),
            Self::Bullseye(_) => Self::Bullseye(Some(extension)),
        }
    }

    fn without_extension(&self) -> Self {
        match self {
            Self::Sid | Self::RCBuggy => *self,
            Self::Trixie(_) => Self::Trixie(None),
            Self::Bookworm(_) => Self::Bookworm(None),
            Self::Bullseye(_) => Self::Bullseye(None),
        }
    }
}

impl Display for Codename {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        match self {
            Self::Sid => write!(f, "sid"),
            Self::Trixie(None) => write!(f, "trixie"),
            Self::Bookworm(None) => write!(f, "bookworm"),
            Self::Bullseye(None) => write!(f, "bullseye"),
            Self::RCBuggy => write!(f, "rc-buggy"),
            Self::Trixie(Some(ext)) => write!(f, "trixie-{ext}"),
            Self::Bookworm(Some(ext)) => write!(f, "bookworm-{ext}"),
            Self::Bullseye(Some(ext)) => write!(f, "bullseye-{ext}"),
        }
    }
}

impl TryFrom<&str> for Codename {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "sid" => Ok(Self::Sid),
            "trixie" => Ok(Self::Trixie(None)),
            "bookworm" => Ok(Self::Bookworm(None)),
            "bullseye" => Ok(Self::Bullseye(None)),
            "rc-buggy" => Ok(Self::RCBuggy),
            _ => {
                let s = value.split_once('-').ok_or(ParseError::InvalidCodename)?;
                let ext = Extension::try_from(s.1)?;
                match s.0 {
                    "trixie" => Ok(Self::Trixie(Some(ext))),
                    "bookworm" => Ok(Self::Bookworm(Some(ext))),
                    "bullseye" => Ok(Self::Bullseye(Some(ext))),
                    _ => Err(ParseError::InvalidCodename),
                }
            }
        }
    }
}

impl FromStr for Codename {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s)
    }
}

impl From<Suite> for Codename {
    fn from(suite: Suite) -> Self {
        match suite {
            Suite::Unstable => Self::Sid,
            Suite::Testing(ext) => Self::Trixie(ext),
            Suite::Stable(ext) => Self::Bookworm(ext),
            Suite::OldStable(ext) => Self::Bullseye(ext),
            Suite::Experimental => Self::RCBuggy,
        }
    }
}

impl From<Codename> for Suite {
    fn from(codename: Codename) -> Self {
        match codename {
            Codename::Sid => Self::Unstable,
            Codename::Trixie(ext) => Self::Testing(ext),
            Codename::Bookworm(ext) => Self::Stable(ext),
            Codename::Bullseye(ext) => Self::OldStable(ext),
            Codename::RCBuggy => Self::Experimental,
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
        deserializer.deserialize_str(TryFromStrVisitor::<Self>::new("a codename"))
    }
}

/// Represents either a suite or codename
///
/// This enum is useful whenever a suite name or codename works
#[derive(Clone, Copy, Debug, Eq)]
pub enum SuiteOrCodename {
    /// A suite
    Suite(Suite),
    /// A codename
    Codename(Codename),
}

impl SuiteOrCodename {
    /// Unstable
    pub const UNSTABLE: Self = Self::Suite(Suite::Unstable);
    /// Testing
    pub const TESTING: Self = Self::Suite(Suite::Testing(None));
    /// Stable
    pub const STABLE: Self = Self::Suite(Suite::Stable(None));
    /// Oldstable
    pub const OLDSTABLE: Self = Self::Suite(Suite::OldStable(None));
    /// Experimental
    pub const EXPERIMENTAL: Self = Self::Suite(Suite::Experimental);
    /// Stable proposed-updates
    pub const STABLE_PU: Self = Self::Suite(Suite::Stable(Some(Extension::ProposedUpdates)));
    /// Oldstable propoused-updates
    pub const OLDSTABLE_PU: Self = Self::Suite(Suite::OldStable(Some(Extension::ProposedUpdates)));
    /// Stable backports
    pub const STABLE_BACKPORTS: Self = Self::Suite(Suite::Stable(Some(Extension::Backports)));
}

impl PartialEq for SuiteOrCodename {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Suite(l0), Self::Suite(r0)) => l0 == r0,
            (Self::Codename(l0), Self::Codename(r0)) => l0 == r0,
            (Self::Suite(l0), Self::Codename(r0)) => Suite::from(*r0) == *l0,
            (Self::Codename(l0), Self::Suite(r0)) => Suite::from(*l0) == *r0,
        }
    }
}

impl Hash for SuiteOrCodename {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        match self {
            Self::Suite(suite) => suite.hash(state),
            Self::Codename(codename) => Suite::from(*codename).hash(state),
        }
    }
}

impl WithExtension for SuiteOrCodename {
    fn with_extension(&self, extension: Extension) -> Self {
        match self {
            Self::Suite(suite) => Self::Suite(suite.with_extension(extension)),
            Self::Codename(suite) => Self::Codename(suite.with_extension(extension)),
        }
    }

    fn without_extension(&self) -> Self {
        match self {
            Self::Suite(suite) => Self::Suite(suite.without_extension()),
            Self::Codename(suite) => Self::Codename(suite.without_extension()),
        }
    }
}

impl From<Codename> for SuiteOrCodename {
    fn from(codename: Codename) -> Self {
        Self::Codename(codename)
    }
}

impl From<Suite> for SuiteOrCodename {
    fn from(suite: Suite) -> Self {
        Self::Suite(suite)
    }
}

impl TryFrom<&str> for SuiteOrCodename {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match Suite::try_from(value) {
            Ok(suite) => Ok(Self::Suite(suite)),
            Err(_) => match Codename::try_from(value) {
                Ok(codename) => Ok(Self::Codename(codename)),
                Err(_) => Err(ParseError::InvalidSuiteOrCodename),
            },
        }
    }
}

impl FromStr for SuiteOrCodename {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s)
    }
}

impl Display for SuiteOrCodename {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Suite(suite) => suite.fmt(f),
            Self::Codename(codename) => codename.fmt(f),
        }
    }
}

impl From<SuiteOrCodename> for Suite {
    fn from(value: SuiteOrCodename) -> Self {
        match value {
            SuiteOrCodename::Suite(suite) => suite,
            SuiteOrCodename::Codename(codename) => Self::from(codename),
        }
    }
}

impl From<SuiteOrCodename> for Codename {
    fn from(value: SuiteOrCodename) -> Self {
        match value {
            SuiteOrCodename::Suite(suite) => Self::from(suite),
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
        deserializer.deserialize_str(TryFromStrVisitor::<Self>::new("a suite or a codename"))
    }
}

/// Allowed values of the multi-arch field
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Hash)]
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
            Self::Allowed => "allowed",
            Self::Foreign => "foreign",
            Self::No => "no",
            Self::Same => "same",
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
            "allowed" => Ok(Self::Allowed),
            "foreign" => Ok(Self::Foreign),
            "no" => Ok(Self::No),
            "same" => Ok(Self::Same),
            _ => Err(ParseError::InvalidMultiArch),
        }
    }
}

impl FromStr for MultiArch {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s)
    }
}

/// Debian archive components
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Hash)]
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
            Self::Main => "main",
            Self::Contrib => "contrib",
            Self::NonFree => "non-free",
            Self::NonFreeFirmware => "non-free-firmware",
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
            "main" => Ok(Self::Main),
            "contrib" => Ok(Self::Contrib),
            "non-free" => Ok(Self::NonFree),
            "non-free-firmware" => Ok(Self::NonFreeFirmware),
            _ => Err(ParseError::InvalidComponent),
        }
    }
}

impl FromStr for Component {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s)
    }
}

#[cfg(test)]
mod test {
    use std::hash::DefaultHasher;

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
    fn suite_or_codename_eq() {
        assert_eq!(
            SuiteOrCodename::UNSTABLE,
            SuiteOrCodename::Codename(Codename::Sid)
        );
        assert_eq!(
            SuiteOrCodename::STABLE_PU,
            SuiteOrCodename::Codename(Codename::Bookworm(Some(Extension::ProposedUpdates)))
        );
    }

    #[test]
    fn suite_or_codename_hash() {
        let mut hasher_1 = DefaultHasher::new();
        let mut hasher_2 = DefaultHasher::new();

        SuiteOrCodename::UNSTABLE.hash(&mut hasher_1);
        SuiteOrCodename::Codename(Codename::Sid).hash(&mut hasher_2);
        assert_eq!(hasher_1.finish(), hasher_2.finish());
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
