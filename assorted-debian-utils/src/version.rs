// Copyright 2022 Sebastian Ramacher
// SPDX-License-Identifier: LGPL-3.0-or-later

//! # Version handling
//!
//! This module handles versions of Debian packages.
//!
//! ```
//! use assorted_debian_utils::version::PackageVersion;
//!
//! let ver1 = PackageVersion::new(None, "1.0", Some("2")).expect("Failed to construct version");
//! assert_eq!(ver1.to_string(), "1.0-2");
//! assert!(!ver1.has_epoch());
//! assert!(!ver1.is_native());
//!
//! let ver2 = PackageVersion::new(Some(1), "0.2", Some("1.1")).expect("Failed to construct version");
//! assert_eq!(ver2.to_string(), "1:0.2-1.1");
//! assert!(ver2.has_epoch());
//! assert!(!ver2.is_native());
//!
//! assert!(ver1 < ver2);
//! assert_eq!(ver1, PackageVersion::new(Some(0), "1.0", Some("2")).expect("Failed to construct version"));
//! ```

use std::{
    cmp::Ordering,
    fmt::{Display, Formatter},
    hash::{Hash, Hasher},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use crate::ParseError;
use crate::utils::TryFromStrVisitor;

/// Compare non-digits part of a version
///
/// Non-letters sort before letters, and ~ always sorts first.
fn compare_non_digits(mut lhs: &str, mut rhs: &str) -> Ordering {
    while !lhs.is_empty() || !rhs.is_empty() {
        let (lhs_tilde, lhs_found) = lhs.find('~').map_or((lhs.len(), false), |i| (i, true));
        let (rhs_tilde, rhs_found) = rhs.find('~').map_or((rhs.len(), false), |i| (i, true));
        let c = lhs[..lhs_tilde].cmp(&rhs[..rhs_tilde]);
        if c != Ordering::Equal {
            return c;
        }

        if lhs_found && rhs_found {
            lhs = &lhs[lhs_tilde + 1..];
            rhs = &rhs[rhs_tilde + 1..];
        } else if lhs_found {
            return Ordering::Less;
        } else if rhs_found {
            return Ordering::Greater;
        } else {
            return Ordering::Equal;
        }
    }

    // both lhs and rhs are empty
    Ordering::Equal
}

/// Compare parts of the two versions
fn compare_parts(mut lhs: &str, mut rhs: &str) -> Ordering {
    while !lhs.is_empty() || !rhs.is_empty() {
        // compare initial non-digits
        let lhs_digit_start = lhs.find(|c| char::is_ascii_digit(&c)).unwrap_or(lhs.len());
        let rhs_digit_start = rhs.find(|c| char::is_ascii_digit(&c)).unwrap_or(rhs.len());
        let c = compare_non_digits(&lhs[..lhs_digit_start], &rhs[..rhs_digit_start]);
        if c != Ordering::Equal {
            return c;
        }
        lhs = &lhs[lhs_digit_start..];
        rhs = &rhs[rhs_digit_start..];

        // compare initial digits
        let lhs_digit_end = lhs.find(|c| !char::is_ascii_digit(&c)).unwrap_or(lhs.len());
        let rhs_digit_end = rhs.find(|c| !char::is_ascii_digit(&c)).unwrap_or(rhs.len());
        let c = lhs[..lhs_digit_end]
            .parse::<u64>()
            .unwrap_or(0)
            .cmp(&rhs[..rhs_digit_end].parse::<u64>().unwrap_or(0));
        if c != Ordering::Equal {
            return c;
        }
        lhs = &lhs[lhs_digit_end..];
        rhs = &rhs[rhs_digit_end..];
    }

    // both lhs and rhs are empty
    Ordering::Equal
}

/// Version errors
#[derive(Clone, Copy, Debug, Error)]
pub enum VersionError {
    #[error("invalid epoch")]
    /// Epoch is invalid
    InvalidEpoch,
    #[error("invalid upstream version")]
    /// Upstream version is invalid
    InvalidUpstreamVersion,
    #[error("invalid Debian revision")]
    /// Debian revision is invalid
    InvalidDebianRevision,
}

/// A version number of a Debian package
///
/// Version numbers consists of three components:
/// * an optional epoch
/// * the upstream version
/// * an optional debian revision
#[derive(Clone, Debug)]
pub struct PackageVersion {
    /// The (optional) epoch
    pub(crate) epoch: Option<u32>,
    /// The upstream version
    pub(crate) upstream_version: String,
    /// The (optional) Debian revision
    pub(crate) debian_revision: Option<String>,
}

impl PackageVersion {
    /// Create a new version struct from the individual components.
    pub fn new(
        epoch: Option<u32>,
        upstream_version: &str,
        debian_revision: Option<&str>,
    ) -> Result<Self, VersionError> {
        // Upstream version may consist of alphanumeric characters and ., +, ~, - (if the revision is non-empty), : (if the epoch is non-empty)
        if upstream_version.is_empty()
            || upstream_version.chars().any(|c| {
                !(c.is_alphanumeric()
                    || ".+~".contains(c)
                    || (debian_revision.is_some() && c == '-')
                    || (epoch.is_some() && c == ':'))
            })
        {
            return Err(VersionError::InvalidUpstreamVersion);
        }

        // Debian revision may consist of alphanumeric characters and ., +, ~
        if let Some(rev) = debian_revision {
            if rev.is_empty()
                || rev
                    .chars()
                    .any(|c| !c.is_alphanumeric() && !".+~".contains(c))
            {
                return Err(VersionError::InvalidDebianRevision);
            }
        }

        Ok(Self {
            epoch,
            upstream_version: String::from(upstream_version),
            debian_revision: debian_revision.map(String::from),
        })
    }

    /// Returns whether version is a native version, i.e., there is no revision.
    pub fn is_native(&self) -> bool {
        self.debian_revision.is_none()
    }

    /// Return whether the version has an epoch.
    pub fn has_epoch(&self) -> bool {
        self.epoch.is_some()
    }

    /// Return epoch of 0 if none set.
    pub fn epoch_or_0(&self) -> u32 {
        self.epoch.unwrap_or(0)
    }

    /// Return whether this version has a binNMU version, i.e., ends in +bX for some integer X.
    pub fn has_binnmu_version(&self) -> bool {
        self.binnmu_version().is_some()
    }

    /// Return binNMU version if available.
    pub fn binnmu_version(&self) -> Option<u32> {
        self.debian_revision
            .as_ref()
            .map_or(&self.upstream_version, |v| v)
            .rsplit_once("+b")
            .and_then(|(_, binnmu_version)| binnmu_version.parse().ok())
    }

    /// Obtain version without the binNMU version.
    pub fn without_binnmu_version(mut self) -> Self {
        if let Some(revision) = self.debian_revision.as_mut() {
            if let Some(index) = revision.rfind("+b") {
                revision.truncate(index);
            }
        } else if let Some(index) = self.upstream_version.rfind("+b") {
            self.upstream_version.truncate(index);
        }
        self
    }

    /// Obtain version without the binNMU version.
    pub fn clone_without_binnmu_version(&self) -> Self {
        self.clone().without_binnmu_version()
    }
}

impl PartialOrd for PackageVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PackageVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.epoch_or_0().cmp(&other.epoch_or_0()) {
            Ordering::Equal => {}
            v => return v,
        };

        match compare_parts(&self.upstream_version, &other.upstream_version) {
            Ordering::Equal => {}
            v => return v,
        };

        match (&self.debian_revision, &other.debian_revision) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (Some(lhs), Some(rhs)) => compare_parts(lhs, rhs),
        }
    }
}

impl PartialEq for PackageVersion {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for PackageVersion {}

impl PartialEq<&str> for PackageVersion {
    fn eq(&self, other: &&str) -> bool {
        self.partial_cmp(other) == Some(Ordering::Equal)
    }
}

impl PartialOrd<&str> for PackageVersion {
    fn partial_cmp(&self, other: &&str) -> Option<Ordering> {
        let rhs = Self::try_from(*other);
        match rhs {
            Err(_) => None,
            Ok(rhs) => self.partial_cmp(&rhs),
        }
    }
}

impl TryFrom<&str> for PackageVersion {
    type Error = ParseError;

    fn try_from(mut value: &str) -> Result<Self, Self::Error> {
        let epoch = if let Some((epoch_str, new_value)) = value.split_once(':') {
            value = new_value;
            Some(
                epoch_str
                    .parse::<u32>()
                    .map_err(|_| ParseError::InvalidVersion(VersionError::InvalidEpoch))?,
            )
        } else {
            None
        };

        let debian_revision = if let Some((new_value, debian_revision_str)) = value.rsplit_once('-')
        {
            value = new_value;
            Some(debian_revision_str)
        } else {
            None
        };

        Self::new(epoch, value, debian_revision).map_err(ParseError::InvalidVersion)
    }
}

impl Display for PackageVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(epoch) = self.epoch {
            write!(f, "{epoch}:")?;
        }
        write!(f, "{}", self.upstream_version)?;
        if let Some(debian_revision) = &self.debian_revision {
            write!(f, "-{debian_revision}")?;
        }
        Ok(())
    }
}

impl Serialize for PackageVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for PackageVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(TryFromStrVisitor::new("a package version"))
    }
}

impl Hash for PackageVersion {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.epoch_or_0().hash(state);
        self.upstream_version.hash(state);
        self.debian_revision.hash(state);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn conversion() {
        let version = PackageVersion::try_from("2:1.0+dfsg-1").unwrap();
        assert_eq!(version.epoch, Some(2));
        assert_eq!(version.upstream_version, "1.0+dfsg");
        assert_eq!(version.debian_revision, Some("1".into()));
    }

    #[test]
    fn invalid_epoch() {
        assert!(PackageVersion::try_from("-1:1.0-1").is_err());
        assert!(PackageVersion::try_from(":1.0-1").is_err());
        assert!(PackageVersion::try_from("a1:1.0-1").is_err());
    }

    #[test]
    fn invalid_upstream_version() {
        assert!(PackageVersion::try_from("-1").is_err());
        assert!(PackageVersion::try_from("0:-1").is_err());
        assert!(PackageVersion::new(None, "1:2", None).is_err());
        assert!(PackageVersion::new(None, "1-2", None).is_err());
    }

    #[test]
    fn multi_dash() {
        let version = PackageVersion::try_from("1.0-2-1").unwrap();
        assert_eq!(version.epoch, None);
        assert_eq!(version.upstream_version, "1.0-2");
        assert_eq!(version.debian_revision.unwrap(), "1");
    }

    #[test]
    fn multi_colon() {
        let version = PackageVersion::try_from("1:1.0:2-1").unwrap();
        assert_eq!(version.epoch.unwrap(), 1);
        assert_eq!(version.upstream_version, "1.0:2");
        assert_eq!(version.debian_revision.unwrap(), "1");
    }

    #[test]
    fn binnum() {
        let version = PackageVersion::try_from("1.0-1").unwrap();
        assert!(!version.has_binnmu_version());
        assert_eq!(version.binnmu_version(), None);

        let version = PackageVersion::try_from("1.0-1+b1").unwrap();
        assert!(version.has_binnmu_version());
        assert_eq!(version.binnmu_version(), Some(1u32));
    }

    #[test]
    fn strip_binnum() {
        let version = PackageVersion::try_from("1.0-1+b1").unwrap();
        let version = version.without_binnmu_version();
        assert_eq!(version, PackageVersion::try_from("1.0-1").unwrap());

        assert!(!version.has_binnmu_version());
        assert_eq!(version.binnmu_version(), None);
    }

    #[test]
    fn compare_non_digits_invariants() {
        assert_eq!(compare_non_digits("~~", "~~a"), Ordering::Less);
        assert_eq!(compare_non_digits("~~a", "~"), Ordering::Less);
        assert_eq!(compare_non_digits("~", ""), Ordering::Less);
        assert_eq!(compare_non_digits("", "a"), Ordering::Less);
    }

    #[test]
    fn epoch_compare() {
        let version1 = PackageVersion::try_from("2.0-1").unwrap();
        let version2 = PackageVersion::try_from("2:1.0+dfsg-1").unwrap();

        assert!(version2.has_epoch());
        assert!(!version1.has_epoch());
        assert!(version1 < version2);
    }

    #[test]
    fn zero_epoch_compare() {
        let version1 = PackageVersion::try_from("2.0-1").unwrap();
        let version2 = PackageVersion::try_from("0:2.0-1").unwrap();
        assert_eq!(version1, version2);
    }

    #[test]
    fn equal_compare() {
        let version1 = PackageVersion::try_from("2.0-1").unwrap();
        assert_eq!(version1, version1);

        let version1 = PackageVersion::try_from("2a.0-1").unwrap();
        assert_eq!(version1, version1);

        let version1 = PackageVersion::try_from("2+dfsg1-1").unwrap();
        assert_eq!(version1, version1);
    }

    #[test]
    fn tilde_plus_compare() {
        let version1 = PackageVersion::try_from("2.0~dfsg-1").unwrap();
        let version2 = PackageVersion::try_from("2.0-1").unwrap();
        assert!(version1 < version2);

        let version2 = PackageVersion::try_from("2.0+dfsg-1").unwrap();
        assert!(version1 < version2);

        let version1 = PackageVersion::try_from("2.0-1").unwrap();
        assert!(version1 < version2);

        let version1 = PackageVersion::try_from("2+dfsg1-1").unwrap();
        let version2 = PackageVersion::try_from("2+dfsg2-1").unwrap();
        assert!(version1 < version2);

        let version1 = PackageVersion::try_from("2+dfsg1-1").unwrap();
        let version2 = PackageVersion::try_from("2.1-1").unwrap();
        assert!(version1 < version2);
    }

    #[test]
    fn letters_compare() {
        let version1 = PackageVersion::try_from("2dfsg-1").unwrap();
        let version2 = PackageVersion::try_from("2-1").unwrap();
        assert!(version1 > version2);
    }

    #[test]
    fn less_compare() {
        let version1 = PackageVersion::try_from("2-1").unwrap();
        let version2 = PackageVersion::try_from("2.0-1").unwrap();
        assert!(version1 < version2);
    }

    #[test]
    fn native_version_binnmu() {
        let version1 = PackageVersion::try_from("2+b1").unwrap();
        let version2 = PackageVersion::try_from("2").unwrap();
        assert!(version1.has_binnmu_version());
        assert_eq!(version1.binnmu_version(), Some(1));
        assert!(!version2.has_binnmu_version());
        assert_eq!(version1.without_binnmu_version(), version2);
    }
}
