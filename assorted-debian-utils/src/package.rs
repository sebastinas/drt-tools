// Copyright 2025-2026 Sebastian Ramacher
// SPDX-License-Identifier: LGPL-3.0-or-later

//! # Helpers to handle Debian packages
//!
//! These helpers includes abstractions to check the validity of Debian packages names.

use std::{
    borrow::Borrow,
    collections::HashSet,
    fmt::{self, Display},
    str::FromStr,
};

use serde::Deserialize;
use thiserror::Error;

use crate::{
    ParseError,
    architectures::{Architecture, ArchitectureTuple},
    build_profile::{ProfileFormula, ProfileName},
    utils::TryFromStrVisitor,
    version::PackageVersion,
};

fn check_package_name(package: &str) -> Result<(), PackageError> {
    // package names must be at least 2 characters long
    if package.len() < 2 {
        return Err(PackageError::InvalidNameLength);
    }

    if !package
        .chars()
        .enumerate()
        .all(|(i, c)| c.is_ascii_lowercase() || c.is_ascii_digit() || (i > 0 && ".+-".contains(c)))
    {
        Err(PackageError::InvalidName)
    } else {
        Ok(())
    }
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

impl Borrow<str> for PackageName {
    fn borrow(&self) -> &str {
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

/// Represents the comparison operation for a version relationship
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Relationship {
    /// Less than
    Less,
    /// Less or equal
    LessEqual,
    /// Equal
    Equal,
    /// Greater than
    Greater,
    /// Greater or equal
    GreaterEqual,
}

impl AsRef<str> for Relationship {
    fn as_ref(&self) -> &str {
        match self {
            Self::Less => "<<",
            Self::LessEqual => "<=",
            Self::Equal => "=",
            Self::Greater => ">>",
            Self::GreaterEqual => ">=",
        }
    }
}

impl Display for Relationship {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

impl TryFrom<&str> for Relationship {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "<<" => Ok(Self::Less),
            "<=" => Ok(Self::LessEqual),
            "=" => Ok(Self::Equal),
            ">>" => Ok(Self::Greater),
            ">=" => Ok(Self::GreaterEqual),
            _ => Err(ParseError::InvalidRelationship),
        }
    }
}

impl FromStr for Relationship {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s)
    }
}

/// Represents a version relationship, i.e., it consists of a comparison operator and a version
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VersionRelationship {
    /// The version
    pub version: PackageVersion,
    /// Comparison operator
    pub relation: Relationship,
}

impl Display for VersionRelationship {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.relation, self.version)
    }
}

impl TryFrom<&str> for VersionRelationship {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        // A whitespace seperator is optional, hence search for the first
        // alphanumeric character as start of the version.
        if let Some(pos) = value.find(char::is_alphanumeric) {
            let (rel, ver) = value.split_at(pos);
            let rel = rel.trim_end();

            let relation = rel.try_into()?;
            let version = ver.try_into()?;
            Ok(Self { version, relation })
        } else {
            Err(ParseError::InvalidRelationship)
        }
    }
}

/// List of architecture restrictions
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchitectureRestriction {
    positive_list: HashSet<ArchitectureTuple>,
    negative_list: HashSet<ArchitectureTuple>,
}

impl ArchitectureRestriction {
    /// Check whether a given architectures satisfies the architecture restrictions.
    pub fn satisfied_by(&self, architecture: Architecture) -> bool {
        let at = ArchitectureTuple::from(architecture);
        (self.positive_list.is_empty() || self.positive_list.iter().any(|a| a.contains(at)))
            && !self.negative_list.iter().any(|a| a.contains(at))
    }
}

impl TryFrom<&str> for ArchitectureRestriction {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let mut positive_list = HashSet::default();
        let mut negative_list = HashSet::default();

        for value in value.split(' ') {
            if let Some(value) = value.strip_prefix('!') {
                negative_list.insert(
                    value
                        .try_into()
                        .map_err(|_| ParseError::InvalidRelationship)?,
                );
            } else {
                positive_list.insert(
                    value
                        .try_into()
                        .map_err(|_| ParseError::InvalidRelationship)?,
                );
            }
        }

        Ok(Self {
            positive_list,
            negative_list,
        })
    }
}

/// Relationship (Build-Depends, Depends, Recommends, Suggests, etc) on some package of the
/// form `package (version) [arch] <profile>` whereas everything except the
/// package is optional.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageRelationship {
    /// Package part
    pub package: PackageName,
    /// Version relationship
    pub version_relation: Option<VersionRelationship>,
    /// Architecture restrictions
    pub architecture_restrictions: Option<ArchitectureRestriction>,
    /// Build profile clauses
    pub build_profiles: Option<ProfileFormula>,
}

impl PackageRelationship {
    pub fn valid_for_architecture(&self, architecture: Architecture) -> bool {
        self.architecture_restrictions
            .as_ref()
            .is_none_or(|restriction| restriction.satisfied_by(architecture))
    }

    pub fn valid_for_build_profiles(&self, build_profiles: &[ProfileName]) -> bool {
        self.build_profiles
            .as_ref()
            .is_none_or(|formula| formula.satisfied_by(build_profiles))
    }
}

impl Display for PackageRelationship {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.version_relation {
            Some(vr) => write!(f, "{} ({})", self.package, vr),
            None => write!(f, "{}", self.package),
        }
    }
}

impl TryFrom<&str> for PackageRelationship {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        // start from the end: build profile
        let (value, build_profiles) = if let Some(index) = value.find('<') {
            if !value.ends_with('>') {
                return Err(ParseError::InvalidRelationship);
            }
            let (value, profile) = value.split_at(index);
            (value.trim_end(), Some(ProfileFormula::try_from(profile)?))
        } else {
            (value, None)
        };
        // architecture restrictions
        let (value, architecture_restrictions) =
            if let Some((value, architecture_restriction)) = value.split_once('[') {
                if !architecture_restriction.ends_with(']') {
                    return Err(ParseError::InvalidRelationship);
                }
                (
                    value.trim_end(),
                    Some(ArchitectureRestriction::try_from(
                        &architecture_restriction[..architecture_restriction.len() - 1],
                    )?),
                )
            } else {
                (value, None)
            };
        // package name and version relation
        let (package, version_relation) =
            if let Some((package, version_relation)) = value.split_once('(') {
                if !version_relation.ends_with(')') {
                    return Err(ParseError::InvalidRelationship);
                }
                (
                    PackageName::try_from(package.trim_end())?,
                    Some(VersionRelationship::try_from(
                        &version_relation[..version_relation.len() - 1],
                    )?),
                )
            } else {
                (PackageName::try_from(value)?, None)
            };

        Ok(Self {
            package,
            version_relation,
            architecture_restrictions,
            build_profiles,
        })
    }
}

impl FromStr for PackageRelationship {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s)
    }
}

impl<'de> Deserialize<'de> for PackageRelationship {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(TryFromStrVisitor::new("a package relationship"))
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

    #[test]
    fn architecture_restriction() {
        let ar = ArchitectureRestriction::try_from("any-amd64").unwrap();
        assert!(ar.satisfied_by(Architecture::Amd64));
        assert!(ar.satisfied_by(Architecture::HurdAmd64));
        assert!(!ar.satisfied_by(Architecture::I386));
        assert!(ar.satisfied_by(Architecture::X32));
        assert!(!ar.satisfied_by(Architecture::HurdI386));

        let ar = ArchitectureRestriction::try_from("any-amd64 !x32").unwrap();
        assert!(ar.satisfied_by(Architecture::Amd64));
        assert!(ar.satisfied_by(Architecture::HurdAmd64));
        assert!(!ar.satisfied_by(Architecture::I386));
        assert!(!ar.satisfied_by(Architecture::X32));
        assert!(!ar.satisfied_by(Architecture::HurdI386));

        let ar = ArchitectureRestriction::try_from("!x32").unwrap();
        assert!(ar.satisfied_by(Architecture::Amd64));
        assert!(ar.satisfied_by(Architecture::HurdAmd64));
        assert!(ar.satisfied_by(Architecture::I386));
        assert!(!ar.satisfied_by(Architecture::X32));
        assert!(ar.satisfied_by(Architecture::HurdI386));
    }

    #[test]
    fn package_relationship() {
        let package_relationship =
            PackageRelationship::try_from("zathura (>= 1.0~) [amd64]").unwrap();
        assert_eq!(package_relationship.package, "zathura");
        assert_eq!(
            package_relationship.version_relation,
            Some(VersionRelationship {
                version: "1.0~".try_into().unwrap(),
                relation: Relationship::GreaterEqual
            })
        );

        let package_relationship =
            PackageRelationship::try_from("zathura(>= 1.0~)[amd64]").unwrap();
        assert_eq!(package_relationship.package, "zathura");
        assert_eq!(
            package_relationship.version_relation,
            Some(VersionRelationship {
                version: "1.0~".try_into().unwrap(),
                relation: Relationship::GreaterEqual
            })
        );

        let package_relationship =
            PackageRelationship::try_from("zathura (>= 1.0~) [amd64] <!nocheck>").unwrap();
        assert_eq!(package_relationship.package, "zathura");
        assert_eq!(
            package_relationship.version_relation,
            Some(VersionRelationship {
                version: "1.0~".try_into().unwrap(),
                relation: Relationship::GreaterEqual
            })
        );
        assert!(package_relationship.valid_for_architecture(Architecture::Amd64));
        assert!(!package_relationship.valid_for_architecture(Architecture::I386));
        assert!(
            package_relationship
                .valid_for_build_profiles(&[ProfileName::try_from("nogir").unwrap()])
        );
        assert!(
            !package_relationship
                .valid_for_build_profiles(&[ProfileName::try_from("nocheck").unwrap()])
        );

        let package_relationship = PackageRelationship::try_from(
            "zathura (>= 1.0~) [amd64] <!nocheck> <!pkg.foo.1 pkg.foo.2>",
        )
        .unwrap();
        assert_eq!(package_relationship.package, "zathura");
        assert_eq!(
            package_relationship.version_relation,
            Some(VersionRelationship {
                version: "1.0~".try_into().unwrap(),
                relation: Relationship::GreaterEqual
            })
        );
        assert!(
            package_relationship
                .valid_for_build_profiles(&[ProfileName::try_from("pkg.foo.2").unwrap()])
        );
        assert!(
            package_relationship
                .valid_for_build_profiles(&[ProfileName::try_from("pkg.foo.1").unwrap(),])
        );
        assert!(!package_relationship.valid_for_build_profiles(&[
            ProfileName::try_from("pkg.foo.1").unwrap(),
            ProfileName::try_from("nocheck").unwrap()
        ]));
    }
}
