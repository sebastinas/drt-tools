// Copyright 2026 Sebastian Ramacher
// SPDX-License-Identifier: LGPL-3.0-or-later

//! # Helpers to handle Build-Profiles
//!
//! These abstractions handle build profile nanems, restrictions, clauses, and formuales.

use std::fmt::{self, Display};

use itertools::Itertools;
use serde::Deserialize;

use crate::{ParseError, package::PackageName, utils::TryFromStrVisitor};

#[derive(Debug, Clone, PartialEq, Eq)]
/// Representation of a build profile name
pub struct ProfileName(String);

impl Display for ProfileName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

// known profile names according to https://wiki.debian.org/BuildProfileSpec?pow_referer=#Registered_profile_names
const REGISTERED_PROFILE_NAMES: [&str; 21] = [
    "cross",
    "ninsttest",
    "nobiarch",
    "nocheck",
    "nocil",
    "nodoc",
    "nogir",
    "nogolang",
    "noguile",
    "nojava",
    "nolua",
    "noocaml",
    "noperl",
    "nopython",
    "noruby",
    "noudeb",
    "nowasm",
    "nowindows",
    "stage1",
    "stage2",
    "upstream-cargo",
];

impl TryFrom<&str> for ProfileName {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if REGISTERED_PROFILE_NAMES.contains(&value) {
            Ok(Self(value.into()))
        } else if let Some((source, profile)) = value
            .strip_prefix("pkg.")
            .and_then(|source_and_profile| source_and_profile.split_once('.'))
        {
            if PackageName::try_from(source).is_ok()
                && profile
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            {
                Ok(Self(value.into()))
            } else {
                Err(ParseError::InvalidProfile)
            }
        } else {
            Err(ParseError::InvalidProfile)
        }
    }
}

impl<'de> Deserialize<'de> for ProfileName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(TryFromStrVisitor::new("a build profile name"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Build profile with an optional negation
pub enum ProfileRestriction {
    /// $profile
    Positive(ProfileName),
    /// !p$rofile
    Negative(ProfileName),
}

impl ProfileRestriction {
    /// Check whether a given build profile names satisfies the restriction.
    pub fn satisfied_by(&self, build_profiles: &[ProfileName]) -> bool {
        match self {
            Self::Positive(profile_name) => build_profiles.contains(profile_name),
            Self::Negative(profile_name) => !build_profiles.contains(profile_name),
        }
    }
}

impl Display for ProfileRestriction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Positive(profile_name) => profile_name.fmt(f),
            Self::Negative(profile_name) => write!(f, "!{}", profile_name),
        }
    }
}

impl TryFrom<&str> for ProfileRestriction {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if let Some(stripped) = value.strip_prefix("!") {
            ProfileName::try_from(stripped).map(Self::Negative)
        } else {
            ProfileName::try_from(value).map(Self::Positive)
        }
    }
}

impl<'de> Deserialize<'de> for ProfileRestriction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(TryFromStrVisitor::new("a build profile restriction"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// A list of profile restrictions, e.g., `<$profile ...>`
pub struct ProfileClause(Vec<ProfileRestriction>);

impl Display for ProfileClause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{}>", self.0.iter().join(" "))
    }
}

impl ProfileClause {
    /// Check whether a given build profile names satisfies the clause.
    pub fn satisfied_by(&self, build_profiles: &[ProfileName]) -> bool {
        self.0
            .iter()
            .all(|restriction| restriction.satisfied_by(build_profiles))
    }
}

impl TryFrom<&str> for ProfileClause {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if let Some(value) = value
            .strip_prefix('<')
            .and_then(|value| value.strip_suffix('>'))
        {
            value
                .split(" ")
                .map(ProfileRestriction::try_from)
                .collect::<Result<Vec<_>, _>>()
                .map(Self)
        } else {
            Err(ParseError::InvalidProfile)
        }
    }
}

impl<'de> Deserialize<'de> for ProfileClause {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(TryFromStrVisitor::new("a build profile clause"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// A list of profile clauses
pub struct ProfileFormula(Vec<ProfileClause>);

impl ProfileFormula {
    /// Check whether a given build profile names satisfies the formula.
    pub fn satisfied_by(&self, build_profiles: &[ProfileName]) -> bool {
        self.0
            .iter()
            .any(|restriction| restriction.satisfied_by(build_profiles))
    }
}

impl Display for ProfileFormula {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.iter().join(" "))
    }
}

impl TryFrom<&str> for ProfileFormula {
    type Error = ParseError;

    fn try_from(mut value: &str) -> Result<Self, Self::Error> {
        let mut clauses = Vec::new();
        while !value.is_empty() {
            if !value.starts_with('<') {
                return Err(ParseError::InvalidProfile);
            }

            let end = value.find('>').ok_or(ParseError::InvalidProfile)?;

            let (clause, remainder) = value.split_at(end + 1);
            clauses.push(ProfileClause::try_from(clause)?);

            // if there is still something left to parse, the next character needs to be a space
            value = if !remainder.is_empty() {
                remainder
                    .strip_prefix(' ')
                    .ok_or(ParseError::InvalidProfile)?
            } else {
                remainder
            };
        }

        if clauses.is_empty() {
            return Err(ParseError::InvalidProfile);
        }

        Ok(Self(clauses))
    }
}

impl<'de> Deserialize<'de> for ProfileFormula {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(TryFromStrVisitor::new("a build profile formula"))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn build_profiles() {
        let nocheck = ProfileName::try_from("nocheck").unwrap();
        let formula = ProfileFormula::try_from("<!nocheck>").unwrap();
        assert_eq!(formula.0[0].0[0], ProfileRestriction::Negative(nocheck));

        assert!(ProfileName::try_from("pkg.foo.1").is_ok());
        assert!(ProfileRestriction::try_from("!pkg.foo.1").is_ok());
        assert!(ProfileClause::try_from("<!pkg.foo.1 pkg.foo.2>").is_ok());
        assert!(ProfileFormula::try_from("<!pkg.foo.1 pkg.foo.2>").is_ok());
    }
}
