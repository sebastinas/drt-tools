// Copyright 2025 Sebastian Ramacher
// SPDX-License-Identifier: LGPL-3.0-or-later

//! # Helpers to handle Debian packages
//!
//! These helpers includes abstractions to check the validity of Debian packages names.

use std::{borrow::Borrow, fmt::Display};

use serde::Deserialize;
use thiserror::Error;

use crate::utils::TryFromStrVisitor;

/// Package errors
#[derive(Clone, Copy, Debug, Error)]
pub enum PackageError {
    #[error("package name too short")]
    /// Package name is too shoort
    InvalidNameLength,
    #[error("invalid package name")]
    /// Package name is invalid
    InvalidName,
}

/// Package name
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PackageName(String);

impl TryFrom<&str> for PackageName {
    type Error = PackageError;

    fn try_from(package: &str) -> Result<Self, Self::Error> {
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

        Ok(Self(package.to_owned()))
    }
}

impl AsRef<str> for PackageName {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl Borrow<str> for PackageName {
    fn borrow(&self) -> &str {
        self.0.borrow()
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
        deserializer.deserialize_str(TryFromStrVisitor::<Self>::new("a package name"))
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
