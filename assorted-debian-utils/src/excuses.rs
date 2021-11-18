// Copyright 2021 Sebastian Ramacher
// SPDX-License-Identifier: LGPL-3.0-or-later

//! # Helpers to handle `excuses.yaml` for testing migration
//!
//! This module provides helpers to deserialize [excuses.yaml](https://release.debian.org/britney/excuses.yaml)
//! with [serde]. Note however, that this module only handles a biased selection of fields.

use crate::architectures::Architecture;
use chrono::{DateTime, TimeZone, Utc};
use serde::de;
use serde::Deserialize;
use std::{collections::HashMap, fmt, io};

fn deserialize_datetime<'de, D>(deserializer: D) -> std::result::Result<DateTime<Utc>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct DateTimeVisitor;

    impl<'de> de::Visitor<'de> for DateTimeVisitor {
        type Value = DateTime<Utc>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            write!(
                formatter,
                "a date and time formatted as %Y-%m-%d %H:%M:%S%:f"
            )
        }

        fn visit_str<E>(self, s: &str) -> std::result::Result<Self::Value, E>
        where
            E: de::Error,
        {
            match Utc.datetime_from_str(s, "%Y-%m-%d %H:%M:%S%.f") {
                Ok(dt) => Ok(dt),
                Err(_) => Err(de::Error::invalid_value(de::Unexpected::Str(s), &self)),
            }
        }
    }

    deserializer.deserialize_str(DateTimeVisitor)
}

/// The excuses.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Excuses {
    #[serde(deserialize_with = "deserialize_datetime")]
    pub generated_date: DateTime<Utc>,
    pub sources: Vec<ExcusesItem>,
}

/// A policy's verdict
#[derive(Debug, Deserialize, PartialEq)]
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
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Component {
    Main,
    Contrib,
    #[serde(rename = "non-free")]
    NonFree,
}

/// Age policy info
#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct AgeInfo {
    pub age_requirement: u32,
    pub current_age: u32,
    pub verdict: Verdict,
}

/// Catch-all policy info
#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct UnspecfiedPolicyInfo {
    pub verdict: Verdict,
}

/// Built-on-build policy info
#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BuiltOnBuildd {
    pub signed_by: HashMap<Architecture, Option<String>>,
    pub verdict: Verdict,
}

/// Collected policy infos
#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PolicyInfo {
    pub age: Option<AgeInfo>,
    pub builtonbuildd: Option<BuiltOnBuildd>,
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
#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MissingBuilds {
    pub on_architectures: Vec<Architecture>,
}

/// A source package's excuses
#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ExcusesItem {
    pub is_candidate: bool,
    pub new_version: String,
    pub old_version: String,
    pub item_name: String,
    pub source: String,
    pub invalidated_by_other_package: Option<bool>,
    pub component: Option<Component>,
    pub missing_builds: Option<MissingBuilds>,
    #[serde(rename = "policy_info")]
    pub policy_info: Option<PolicyInfo>,
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
