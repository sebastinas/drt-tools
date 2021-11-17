use crate::architectures::Architecture;
use serde::Deserialize;
use std::{collections::HashMap, io};

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Excuses {
    pub generated_date: String,
    pub sources: Vec<ExcusesItem>,
}

/// A policy's verdict
#[derive(Debug, Deserialize, PartialEq)]
pub enum Verdict {
    #[serde(rename = "PASS")]
    Pass,
    #[serde(rename = "PASS_HINTED")]
    PassHinted,
    #[serde(rename = "REJECTED_NEEDS_APPROVAL")]
    RejectedNeedsApproval,
    #[serde(rename = "REJECTED_PERMANENTLY")]
    RejectedPermanently,
    #[serde(rename = "REJECTED_TEMPORARILY")]
    RejectedTemporarily,
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

/// The collectedp olicy infos
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

pub type Result<T> = serde_yaml::Result<T>;

/// Read excuses from a reader
pub fn from_reader(reader: impl io::Read) -> Result<Excuses> {
    serde_yaml::from_reader(reader)
}

/// Read excuses from a string
pub fn from_str(data: &str) -> Result<Excuses> {
    serde_yaml::from_str(data)
}
