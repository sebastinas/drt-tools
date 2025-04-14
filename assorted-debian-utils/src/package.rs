// Copyright 2025 Sebastian Ramacher
// SPDX-License-Identifier: LGPL-3.0-or-later

//! # Helpers to handle Debian packages
//!
//! These helpers includes abstractions to check the validity of Debian packages names.

use std::fmt::Display;

use serde::Deserialize;
use thiserror::Error;

use crate::{utils::TryFromStrVisitor, version::PackageVersion};

fn check_package_name(package: &str) -> Result<(), PackageError> {
    // package names must be at least 2 characters long
    if package.len() < 2 {
        return Err(PackageError::InvalidNameLength);
    }

    if !package.chars().enumerate().all(|(i, c)| {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            return true;
        }
        i > 0 && ".+-".contains(c)
    }) {
        return Err(PackageError::InvalidName);
    }

    Ok(())
}

/// Package errors
#[derive(Clone, Copy, Debug, Error)]
pub enum PackageError {
    #[error("package name too short")]
    /// Package name is too short
    InvalidNameLength,
    #[error("package name contains invalid character")]
    /// Package name is invalid
    InvalidName,
}

/// Package name
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PackageName(String);

impl TryFrom<&str> for PackageName {
    type Error = PackageError;

    fn try_from(package: &str) -> Result<Self, Self::Error> {
        check_package_name(package).map(|_| Self(package.to_owned()))
    }
}

impl TryFrom<String> for PackageName {
    type Error = PackageError;

    fn try_from(package: String) -> Result<Self, Self::Error> {
        check_package_name(&package).map(|_| Self(package))
    }
}

impl AsRef<str> for PackageName {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl PartialEq<&str> for PackageName {
    fn eq(&self, other: &&str) -> bool {
        self.0.eq(other)
    }
}

impl PartialEq<String> for PackageName {
    fn eq(&self, other: &String) -> bool {
        self.0.eq(other)
    }
}

impl Display for PackageName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<'de> Deserialize<'de> for PackageName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(TryFromStrVisitor::new("a package name"))
    }
}

/// A package together with its version
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VersionedPackage {
    /// The package name
    pub package: PackageName,
    /// The package version
    pub version: PackageVersion,
}

impl AsRef<PackageName> for VersionedPackage {
    fn as_ref(&self) -> &PackageName {
        &self.package
    }
}

impl AsRef<PackageVersion> for VersionedPackage {
    fn as_ref(&self) -> &PackageVersion {
        &self.version
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn valid_package_names() {
        assert!(PackageName::try_from("zathura").is_ok());
        assert!(PackageName::try_from("0ad").is_ok());
        assert!(PackageName::try_from("zathura-pdf").is_ok());
    }

    #[test]
    fn invalid_package_names() {
        assert!(PackageName::try_from("z").is_err());
        assert!(PackageName::try_from("-ad").is_err());
    }
}
