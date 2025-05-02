// Copyright 2021-2022 Sebastian Ramacher
// SPDX-License-Identifier: LGPL-3.0-or-later

//! # Helpers to handle `excuses.yaml` for testing migration
//!
//! This module provides helpers to deserialize [excuses.yaml](https://release.debian.org/britney/excuses.yaml)
//! with [serde]. Note however, that this module only handles a biased selection of fields.

use std::{collections::HashMap, fmt::Formatter, io};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, de};
use smallvec::SmallVec;

use crate::{
    architectures::Architecture, archive::Component, package::PackageName, utils::DateTimeVisitor,
    version::PackageVersion,
};

/// Deserialize a datetime string into a `DateTime<Utc>`
fn deserialize_datetime<'de, D>(deserializer: D) -> std::result::Result<DateTime<Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_str(DateTimeVisitor("%Y-%m-%d %H:%M:%S%.f%:z"))
}

/// Deserialize a version or '-' as `PackageVersion` or `None`
fn deserialize_version<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<PackageVersion>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Debug)]
    struct Visitor;

    impl de::Visitor<'_> for Visitor {
        type Value = Option<PackageVersion>;

        fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
            write!(formatter, "a package version or '-'")
        }

        fn visit_str<E>(self, s: &str) -> std::result::Result<Self::Value, E>
        where
            E: de::Error,
        {
            if s == "-" {
                Ok(None)
            } else {
                PackageVersion::try_from(s)
                    .map_err(|_| de::Error::invalid_value(de::Unexpected::Str(s), &self))
                    .map(Some)
            }
        }
    }

    deserializer.deserialize_str(Visitor)
}

/// The excuses.
#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Excuses {
    /// Date of the run that produced `excuses.yaml`
    #[serde(deserialize_with = "deserialize_datetime")]
    pub generated_date: DateTime<Utc>,
    /// All excuse items
    ///
    /// While not every excuses item relates to a source package, the field is still named that way in `excuses.yaml`
    pub sources: Vec<ExcusesItem>,
}

/// A policy's verdict
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub enum Verdict {
    /// Policy passed
    #[serde(rename = "PASS")]
    Pass,
    /// Policy passed due to a hint
    #[serde(rename = "PASS_HINTED")]
    PassHinted,
    /// Rejected due to a block hint or because the upload requires explicit approval (e.g.,
    /// uploads to proposed-updates or testing-proposed-updates)
    #[serde(rename = "REJECTED_NEEDS_APPROVAL")]
    RejectedNeedsApproval,
    /// Rejected tu to a permanent issue
    #[serde(rename = "REJECTED_PERMANENTLY")]
    RejectedPermanently,
    /// Rejected due to a transient issue
    #[serde(rename = "REJECTED_TEMPORARILY")]
    RejectedTemporarily,
    /// Rejected, but not able to determine if the issue is transient
    #[serde(rename = "REJECTED_CANNOT_DETERMINE_IF_PERMANENT")]
    RejectedCannotDetermineIfPermanent,
    /// Reject due to another blocking item.
    #[serde(rename = "REJECTED_BLOCKED_BY_ANOTHER_ITEM")]
    RejectedBlockedByAnotherItem,
    /// Reject due to another blocking item.
    #[serde(rename = "REJECTED_WAITING_FOR_ANOTHER_ITEM")]
    RejectedWaitingForAnotherItem,
}

/// Age policy info
#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct AgeInfo {
    /// The required age
    pub age_requirement: u32,
    /// The current age
    pub current_age: u32,
    /// The verdict
    pub verdict: Verdict,
}

/// Catch-all policy info
#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct UnspecfiedPolicyInfo {
    /// The verdict
    pub verdict: Verdict,
}

/// Built-on-buildd policy info
#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BuiltOnBuildd {
    /// The signers for each architecture
    pub signed_by: HashMap<Architecture, Option<String>>,
    /// The verdict
    pub verdict: Verdict,
}

/// Collected policy infos
#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PolicyInfo {
    /// The age policy
    pub age: Option<AgeInfo>,
    /// The buildt-on-buildd policy
    pub builtonbuildd: Option<BuiltOnBuildd>,
    /// The autopkgtest porlicy
    pub autopkgtest: Option<UnspecfiedPolicyInfo>,
    /// All remaining policies
    #[serde(flatten)]
    pub extras: HashMap<String, UnspecfiedPolicyInfo>,
    /*
        autopkgtest: Option<UnspecfiedPolicyInfo>,
        block: Option<UnspecfiedPolicyInfo>,
        build_depends: Option<UnspecfiedPolicyInfo>,
        built_using:  Option<UnspecfiedPolicyInfo>,
        depends: Option<UnspecfiedPolicyInfo>,
        piuparts: Option<UnspecfiedPolicyInfo>,
        rc_bugs: Option<UnspecfiedPolicyInfo>,
    */
}

/// List of missing builds
#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MissingBuilds {
    /// Architectures where builds are missing
    // 16 is arbitrary, but is large enough to hold all current release architectures
    pub on_architectures: SmallVec<[Architecture; 16]>,
}

/// A source package's excuses
#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ExcusesItem {
    /// Maintainer of the package
    pub maintainer: Option<String>,
    /// The item is a candidate for migration
    pub is_candidate: bool,
    /// Version in the source suite, i.e., the version to migrate
    ///
    /// If the value is `None`, the package is being removed.
    #[serde(deserialize_with = "deserialize_version")]
    pub new_version: Option<PackageVersion>,
    /// Version in the target suite
    ///
    /// If the value is `None`, the package is not yet available in the target suite.
    #[serde(deserialize_with = "deserialize_version")]
    pub old_version: Option<PackageVersion>,
    /// Migration item name
    pub item_name: String,
    /// Source package name
    pub source: PackageName,
    /// Migration is blocked by another package
    pub invalidated_by_other_package: Option<bool>,
    /// Component of the source package
    pub component: Option<Component>,
    /// Missing builds
    pub missing_builds: Option<MissingBuilds>,
    /// Policy info
    #[serde(rename = "policy_info")]
    pub policy_info: Option<PolicyInfo>,
    /// The excuses
    pub excuses: Vec<String>,
    /// Combined verdict
    pub migration_policy_verdict: Verdict,
}

impl ExcusesItem {
    /// Excuses item refers to package removal
    pub fn is_removal(&self) -> bool {
        self.new_version.is_none()
    }

    /// Excuses item refers to a binNMU
    pub fn is_binnmu(&self) -> bool {
        self.new_version == self.old_version
    }

    /// Get architecture of the binNMU or `None`
    pub fn binnmu_arch(&self) -> Option<Architecture> {
        self.item_name.split_once('/').map(|(_, arch)| {
            arch.split_once('_')
                .map_or(arch, |(arch, _)| arch)
                .try_into()
                .unwrap()
        })
    }

    /// Excuses item refers to an item in (stable) proposed-updates
    pub fn is_from_pu(&self) -> bool {
        self.item_name.ends_with("_pu")
    }

    /// Excuses item refers to an item in testing-proposed-updates
    pub fn is_from_tpu(&self) -> bool {
        self.item_name.ends_with("_tpu")
    }
}

/// Result type
pub type Result<T> = serde_yaml::Result<T>;

/// Read excuses from a reader
pub fn from_reader(reader: impl io::Read) -> Result<Excuses> {
    serde_yaml::from_reader(reader)
}

/// Read excuses from a string
pub fn from_str(data: &str) -> Result<Excuses> {
    serde_yaml::from_str(data)
}
