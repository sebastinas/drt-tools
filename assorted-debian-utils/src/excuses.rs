// Copyright 2021-2022 Sebastian Ramacher
// SPDX-License-Identifier: LGPL-3.0-or-later

//! # Helpers to handle `excuses.yaml` for testing migration
//!
//! This module provides helpers to deserialize [excuses.yaml](https://release.debian.org/britney/excuses.yaml)
//! with [serde]. Note however, that this module only handles a biased selection of fields.

use std::{collections::HashMap, fmt, io};

use chrono::{DateTime, Utc};
use serde::{de, Deserialize, Deserializer};
use smallvec::SmallVec;

use crate::{
    architectures::{Architecture, RELEASE_ARCHITECTURES},
    archive::Component,
    utils::DateTimeVisitor,
    version::PackageVersion,
};

/// Deserialize a datetime string into a `DateTime<Utc>`
fn deserialize_datetime<'de, D>(deserializer: D) -> std::result::Result<DateTime<Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_str(DateTimeVisitor("%Y-%m-%d %H:%M:%S%.f"))
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

    impl<'de> de::Visitor<'de> for Visitor {
        type Value = Option<PackageVersion>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
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
    pub on_architectures: SmallVec<[Architecture; RELEASE_ARCHITECTURES.len()]>,
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
    pub source: String,
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

#[cfg(test)]
mod test {
    use crate::excuses::Verdict;

    #[test]
    fn deserialize() {
        let data = r##"generated-date: 2022-07-02 20:09:06.890414
sources:
- excuses:
  - 'Migration status for -kalarmcal (4:21.12.3-2 to -): Will attempt migration (Any
    information below is purely informational)'
  - 'Additional info:'
  - ∙ ∙ Package not in unstable, will try to remove
  is-candidate: true
  item-name: -kalarmcal
  maintainer: Debian Qt/KDE Maintainers
  migration-policy-verdict: PASS
  new-version: '-'
  old-version: 4:21.12.3-2
  reason: []
  source: kalarmcal
- detailed-info:
  - Checking build-dependency on amd64
  - Checking build-dependency (indep) on amd64
  excuses:
  - 'Migration status for libmoosex-types-path-class-perl (0.09-1.1 to 0.09-2): Will
    attempt migration (Any information below is purely informational)'
  - 'Additional info:'
  - ∙ ∙ Piuparts tested OK - <a href="https://piuparts.debian.org/sid/source/libm/libmoosex-types-path-class-perl.html">https://piuparts.debian.org/sid/source/libm/libmoosex-types-path-class-perl.html</a>
  - '∙ ∙ autopkgtest for <a href="#libmoosex-types-path-class-perl">libmoosex-types-path-class-perl</a>/0.09-2:
    <a href="https://ci.debian.net/packages/libm/libmoosex-types-path-class-perl/testing/amd64">amd64</a>:
    <a href="https://ci.debian.net/data/autopkgtest/testing/amd64/libm/libmoosex-types-path-class-perl/23220832/log.gz"><span
    style="background:#87d96c">Pass</span></a>, <a href="https://ci.debian.net/packages/libm/libmoosex-types-path-class-perl/testing/arm64">arm64</a>:
    <a href="https://ci.debian.net/data/autopkgtest/testing/arm64/libm/libmoosex-types-path-class-perl/23221438/log.gz"><span
    style="background:#87d96c">Pass</span></a>, <a href="https://ci.debian.net/packages/libm/libmoosex-types-path-class-perl/testing/armhf">armhf</a>:
    <a href="https://ci.debian.net/data/autopkgtest/testing/armhf/libm/libmoosex-types-path-class-perl/23222108/log.gz"><span
    style="background:#87d96c">Pass</span></a>, <a href="https://ci.debian.net/packages/libm/libmoosex-types-path-class-perl/testing/i386">i386</a>:
    <a href="https://ci.debian.net/data/autopkgtest/testing/i386/libm/libmoosex-types-path-class-perl/23222731/log.gz"><span
    style="background:#87d96c">Pass</span></a>, <a href="https://ci.debian.net/packages/libm/libmoosex-types-path-class-perl/testing/ppc64el">ppc64el</a>:
    <a href="https://ci.debian.net/data/autopkgtest/testing/ppc64el/libm/libmoosex-types-path-class-perl/23223345/log.gz"><span
    style="background:#87d96c">Pass</span></a>, <a href="https://ci.debian.net/packages/libm/libmoosex-types-path-class-perl/testing/s390x">s390x</a>:
    <a href="https://ci.debian.net/data/autopkgtest/testing/s390x/libm/libmoosex-types-path-class-perl/23224981/log.gz"><span
    style="background:#87d96c">Pass</span></a>'
  - ∙ ∙ Required age reduced by 3 days because of autopkgtest
  - ∙ ∙ 2 days old (needed 2 days)
  is-candidate: true
  item-name: libmoosex-types-path-class-perl
  maintainer: Debian Perl Group
  migration-policy-verdict: PASS
  new-version: 0.09-2
  old-version: 0.09-1.1
  policy_info:
    age:
      age-requirement: 2
      current-age: 2
      verdict: PASS
    autopkgtest:
      libfcgi-engine-perl/0.22-1.1:
        amd64:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/amd64/libf/libfcgi-engine-perl/23220825/log.gz
        - https://ci.debian.net/packages/libf/libfcgi-engine-perl/testing/amd64
        - null
        - null
        arm64:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/arm64/libf/libfcgi-engine-perl/23221431/log.gz
        - https://ci.debian.net/packages/libf/libfcgi-engine-perl/testing/arm64
        - null
        - null
        armhf:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/armhf/libf/libfcgi-engine-perl/23222101/log.gz
        - https://ci.debian.net/packages/libf/libfcgi-engine-perl/testing/armhf
        - null
        - null
        i386:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/i386/libf/libfcgi-engine-perl/23222724/log.gz
        - https://ci.debian.net/packages/libf/libfcgi-engine-perl/testing/i386
        - null
        - null
        ppc64el:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/ppc64el/libf/libfcgi-engine-perl/23223338/log.gz
        - https://ci.debian.net/packages/libf/libfcgi-engine-perl/testing/ppc64el
        - null
        - null
        s390x:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/s390x/libf/libfcgi-engine-perl/23224974/log.gz
        - https://ci.debian.net/packages/libf/libfcgi-engine-perl/testing/s390x
        - null
        - null
      libgit-pureperl-perl/0.53-2:
        amd64:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/amd64/libg/libgit-pureperl-perl/23220826/log.gz
        - https://ci.debian.net/packages/libg/libgit-pureperl-perl/testing/amd64
        - null
        - null
        arm64:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/arm64/libg/libgit-pureperl-perl/23221432/log.gz
        - https://ci.debian.net/packages/libg/libgit-pureperl-perl/testing/arm64
        - null
        - null
        armhf:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/armhf/libg/libgit-pureperl-perl/23222102/log.gz
        - https://ci.debian.net/packages/libg/libgit-pureperl-perl/testing/armhf
        - null
        - null
        i386:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/i386/libg/libgit-pureperl-perl/23222725/log.gz
        - https://ci.debian.net/packages/libg/libgit-pureperl-perl/testing/i386
        - null
        - null
        ppc64el:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/ppc64el/libg/libgit-pureperl-perl/23223339/log.gz
        - https://ci.debian.net/packages/libg/libgit-pureperl-perl/testing/ppc64el
        - null
        - null
        s390x:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/s390x/libg/libgit-pureperl-perl/23224975/log.gz
        - https://ci.debian.net/packages/libg/libgit-pureperl-perl/testing/s390x
        - null
        - null
      libmagpie-perl/1.163200-3:
        amd64:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/amd64/libm/libmagpie-perl/23220827/log.gz
        - https://ci.debian.net/packages/libm/libmagpie-perl/testing/amd64
        - null
        - null
        arm64:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/arm64/libm/libmagpie-perl/23221433/log.gz
        - https://ci.debian.net/packages/libm/libmagpie-perl/testing/arm64
        - null
        - null
        armhf:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/armhf/libm/libmagpie-perl/23222103/log.gz
        - https://ci.debian.net/packages/libm/libmagpie-perl/testing/armhf
        - null
        - null
        i386:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/i386/libm/libmagpie-perl/23222726/log.gz
        - https://ci.debian.net/packages/libm/libmagpie-perl/testing/i386
        - null
        - null
        ppc64el:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/ppc64el/libm/libmagpie-perl/23223340/log.gz
        - https://ci.debian.net/packages/libm/libmagpie-perl/testing/ppc64el
        - null
        - null
        s390x:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/s390x/libm/libmagpie-perl/23224976/log.gz
        - https://ci.debian.net/packages/libm/libmagpie-perl/testing/s390x
        - null
        - null
      libmoosex-configfromfile-perl/0.14-2:
        amd64:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/amd64/libm/libmoosex-configfromfile-perl/23220828/log.gz
        - https://ci.debian.net/packages/libm/libmoosex-configfromfile-perl/testing/amd64
        - null
        - null
        arm64:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/arm64/libm/libmoosex-configfromfile-perl/23221434/log.gz
        - https://ci.debian.net/packages/libm/libmoosex-configfromfile-perl/testing/arm64
        - null
        - null
        armhf:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/armhf/libm/libmoosex-configfromfile-perl/23222104/log.gz
        - https://ci.debian.net/packages/libm/libmoosex-configfromfile-perl/testing/armhf
        - null
        - null
        i386:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/i386/libm/libmoosex-configfromfile-perl/23222727/log.gz
        - https://ci.debian.net/packages/libm/libmoosex-configfromfile-perl/testing/i386
        - null
        - null
        ppc64el:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/ppc64el/libm/libmoosex-configfromfile-perl/23223341/log.gz
        - https://ci.debian.net/packages/libm/libmoosex-configfromfile-perl/testing/ppc64el
        - null
        - null
        s390x:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/s390x/libm/libmoosex-configfromfile-perl/23224977/log.gz
        - https://ci.debian.net/packages/libm/libmoosex-configfromfile-perl/testing/s390x
        - null
        - null
      libmoosex-configuration-perl/0.2-2:
        amd64:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/amd64/libm/libmoosex-configuration-perl/23220829/log.gz
        - https://ci.debian.net/packages/libm/libmoosex-configuration-perl/testing/amd64
        - null
        - null
        arm64:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/arm64/libm/libmoosex-configuration-perl/23221435/log.gz
        - https://ci.debian.net/packages/libm/libmoosex-configuration-perl/testing/arm64
        - null
        - null
        armhf:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/armhf/libm/libmoosex-configuration-perl/23222105/log.gz
        - https://ci.debian.net/packages/libm/libmoosex-configuration-perl/testing/armhf
        - null
        - null
        i386:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/i386/libm/libmoosex-configuration-perl/23222728/log.gz
        - https://ci.debian.net/packages/libm/libmoosex-configuration-perl/testing/i386
        - null
        - null
        ppc64el:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/ppc64el/libm/libmoosex-configuration-perl/23223342/log.gz
        - https://ci.debian.net/packages/libm/libmoosex-configuration-perl/testing/ppc64el
        - null
        - null
        s390x:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/s390x/libm/libmoosex-configuration-perl/23224978/log.gz
        - https://ci.debian.net/packages/libm/libmoosex-configuration-perl/testing/s390x
        - null
        - null
      libmoosex-daemonize-perl/0.22-1:
        amd64:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/amd64/libm/libmoosex-daemonize-perl/23220830/log.gz
        - https://ci.debian.net/packages/libm/libmoosex-daemonize-perl/testing/amd64
        - null
        - null
        arm64:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/arm64/libm/libmoosex-daemonize-perl/23221436/log.gz
        - https://ci.debian.net/packages/libm/libmoosex-daemonize-perl/testing/arm64
        - null
        - null
        armhf:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/armhf/libm/libmoosex-daemonize-perl/23222106/log.gz
        - https://ci.debian.net/packages/libm/libmoosex-daemonize-perl/testing/armhf
        - null
        - null
        i386:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/i386/libm/libmoosex-daemonize-perl/23222729/log.gz
        - https://ci.debian.net/packages/libm/libmoosex-daemonize-perl/testing/i386
        - null
        - null
        ppc64el:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/ppc64el/libm/libmoosex-daemonize-perl/23223343/log.gz
        - https://ci.debian.net/packages/libm/libmoosex-daemonize-perl/testing/ppc64el
        - null
        - null
        s390x:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/s390x/libm/libmoosex-daemonize-perl/23224979/log.gz
        - https://ci.debian.net/packages/libm/libmoosex-daemonize-perl/testing/s390x
        - null
        - null
      libmoosex-runnable-perl/0.10-1:
        amd64:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/amd64/libm/libmoosex-runnable-perl/23220831/log.gz
        - https://ci.debian.net/packages/libm/libmoosex-runnable-perl/testing/amd64
        - null
        - null
        arm64:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/arm64/libm/libmoosex-runnable-perl/23221437/log.gz
        - https://ci.debian.net/packages/libm/libmoosex-runnable-perl/testing/arm64
        - null
        - null
        armhf:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/armhf/libm/libmoosex-runnable-perl/23222107/log.gz
        - https://ci.debian.net/packages/libm/libmoosex-runnable-perl/testing/armhf
        - null
        - null
        i386:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/i386/libm/libmoosex-runnable-perl/23222730/log.gz
        - https://ci.debian.net/packages/libm/libmoosex-runnable-perl/testing/i386
        - null
        - null
        ppc64el:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/ppc64el/libm/libmoosex-runnable-perl/23223344/log.gz
        - https://ci.debian.net/packages/libm/libmoosex-runnable-perl/testing/ppc64el
        - null
        - null
        s390x:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/s390x/libm/libmoosex-runnable-perl/23224980/log.gz
        - https://ci.debian.net/packages/libm/libmoosex-runnable-perl/testing/s390x
        - null
        - null
      libmoosex-types-path-class-perl/0.09-2:
        amd64:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/amd64/libm/libmoosex-types-path-class-perl/23220832/log.gz
        - https://ci.debian.net/packages/libm/libmoosex-types-path-class-perl/testing/amd64
        - null
        - null
        arm64:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/arm64/libm/libmoosex-types-path-class-perl/23221438/log.gz
        - https://ci.debian.net/packages/libm/libmoosex-types-path-class-perl/testing/arm64
        - null
        - null
        armhf:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/armhf/libm/libmoosex-types-path-class-perl/23222108/log.gz
        - https://ci.debian.net/packages/libm/libmoosex-types-path-class-perl/testing/armhf
        - null
        - null
        i386:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/i386/libm/libmoosex-types-path-class-perl/23222731/log.gz
        - https://ci.debian.net/packages/libm/libmoosex-types-path-class-perl/testing/i386
        - null
        - null
        ppc64el:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/ppc64el/libm/libmoosex-types-path-class-perl/23223345/log.gz
        - https://ci.debian.net/packages/libm/libmoosex-types-path-class-perl/testing/ppc64el
        - null
        - null
        s390x:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/s390x/libm/libmoosex-types-path-class-perl/23224981/log.gz
        - https://ci.debian.net/packages/libm/libmoosex-types-path-class-perl/testing/s390x
        - null
        - null
      libpackage-locator-perl/0.10-3:
        amd64:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/amd64/libp/libpackage-locator-perl/23220833/log.gz
        - https://ci.debian.net/packages/libp/libpackage-locator-perl/testing/amd64
        - null
        - null
        arm64:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/arm64/libp/libpackage-locator-perl/23221439/log.gz
        - https://ci.debian.net/packages/libp/libpackage-locator-perl/testing/arm64
        - null
        - null
        armhf:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/armhf/libp/libpackage-locator-perl/23222109/log.gz
        - https://ci.debian.net/packages/libp/libpackage-locator-perl/testing/armhf
        - null
        - null
        i386:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/i386/libp/libpackage-locator-perl/23222732/log.gz
        - https://ci.debian.net/packages/libp/libpackage-locator-perl/testing/i386
        - null
        - null
        ppc64el:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/ppc64el/libp/libpackage-locator-perl/23223346/log.gz
        - https://ci.debian.net/packages/libp/libpackage-locator-perl/testing/ppc64el
        - null
        - null
        s390x:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/s390x/libp/libpackage-locator-perl/23224982/log.gz
        - https://ci.debian.net/packages/libp/libpackage-locator-perl/testing/s390x
        - null
        - null
      libtest-tempdir-perl/0.11-1:
        amd64:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/amd64/libt/libtest-tempdir-perl/23220834/log.gz
        - https://ci.debian.net/packages/libt/libtest-tempdir-perl/testing/amd64
        - null
        - null
        arm64:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/arm64/libt/libtest-tempdir-perl/23221440/log.gz
        - https://ci.debian.net/packages/libt/libtest-tempdir-perl/testing/arm64
        - null
        - null
        armhf:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/armhf/libt/libtest-tempdir-perl/23222110/log.gz
        - https://ci.debian.net/packages/libt/libtest-tempdir-perl/testing/armhf
        - null
        - null
        i386:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/i386/libt/libtest-tempdir-perl/23222733/log.gz
        - https://ci.debian.net/packages/libt/libtest-tempdir-perl/testing/i386
        - null
        - null
        ppc64el:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/ppc64el/libt/libtest-tempdir-perl/23223347/log.gz
        - https://ci.debian.net/packages/libt/libtest-tempdir-perl/testing/ppc64el
        - null
        - null
        s390x:
        - PASS
        - https://ci.debian.net/data/autopkgtest/testing/s390x/libt/libtest-tempdir-perl/23224983/log.gz
        - https://ci.debian.net/packages/libt/libtest-tempdir-perl/testing/s390x
        - null
        - null
      verdict: PASS
    block:
      verdict: PASS
    build-depends:
      check-build-depends-indep-on-arch: amd64
      check-build-depends-on-arch: amd64
      verdict: PASS
    built-using:
      verdict: PASS
    builtonbuildd:
      signed-by:
        all: buildd_all-x86-grnet-02@buildd.debian.org
      verdict: PASS
    depends:
      verdict: PASS
    implicit-deps:
      implicit-deps:
        broken-binaries: []
      verdict: PASS
    piuparts:
      piuparts-test-url: https://piuparts.debian.org/sid/source/libm/libmoosex-types-path-class-perl.html
      test-results: pass
      verdict: PASS
    rc-bugs:
      shared-bugs: []
      unique-source-bugs: []
      unique-target-bugs: []
      verdict: PASS
  reason: []
  source: libmoosex-types-path-class-perl
        "##;

        let excuses = super::from_str(data).expect("successful parsing of excuses");
        let sources = excuses.sources;
        assert_eq!(sources.len(), 2);

        let kalarm = &sources[0];
        assert!(kalarm.is_removal());

        let moosex = &sources[1];
        assert_eq!(moosex.source, "libmoosex-types-path-class-perl");
        assert!(moosex.is_candidate);
        assert_eq!(
            moosex
                .policy_info
                .as_ref()
                .unwrap()
                .age
                .as_ref()
                .unwrap()
                .verdict,
            Verdict::Pass
        );
    }
}
