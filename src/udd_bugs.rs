// Copyright 2022-2023 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
    io::Read,
};

use anyhow::Result;
use assorted_debian_utils::{
    archive::{Codename, SuiteOrCodename},
    package::PackageName,
};
use serde::{Deserialize, Deserializer, de};

use crate::config::Cache;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Wishlist,
    Normal,
    Important,
    Serious,
    Grave,
    Critical,
}

impl AsRef<str> for Severity {
    fn as_ref(&self) -> &str {
        match self {
            Self::Wishlist => "wishlist",
            Self::Normal => "normal",
            Self::Important => "important",
            Self::Serious => "serious",
            Self::Grave => "grave",
            Self::Critical => "critical",
        }
    }
}

/*
impl Severity {
    fn is_rc(&self) -> bool {
        self >= &Severity::Serious
    }
}
*/

impl Display for Severity {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

/// Helper to parse comma-separated list of package names
struct CommaListVisitor;

impl de::Visitor<'_> for CommaListVisitor {
    type Value = Vec<PackageName>;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        write!(formatter, "a comma-separated list of package names")
    }

    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        s.split(',')
            .map(|a| PackageName::try_from(a).map_err(E::custom))
            .collect()
    }
}

fn deserialize_sources<'de, D>(deserializer: D) -> Result<Vec<PackageName>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_str(CommaListVisitor)
}

#[derive(Clone, Debug, Deserialize)]
pub struct UDDBug {
    pub id: u32,
    #[serde(deserialize_with = "deserialize_sources")]
    pub source: Vec<PackageName>,
    pub severity: Severity,
    pub title: String,
}

#[derive(Default)]
pub struct UDDBugs {
    bugs: Vec<UDDBug>,
    source_index: HashMap<PackageName, Vec<usize>>,
}

impl UDDBugs {
    pub fn load_for_codename(cache: &Cache, suite: SuiteOrCodename) -> Result<Self> {
        let codename = Codename::from(suite);
        load_bugs_from_reader(cache.get_cache_bufreader(format!("udd-ftbfs-bugs-{codename}.yaml"))?)
    }

    fn new(bugs: Vec<UDDBug>) -> Self {
        let mut udd_bugs = Self {
            bugs,
            ..Default::default()
        };

        for (idx, bug) in udd_bugs.bugs.iter().enumerate() {
            for source in &bug.source {
                udd_bugs
                    .source_index
                    .entry(source.clone())
                    .or_default()
                    .push(idx);
            }
        }

        udd_bugs
    }

    pub fn bugs_for_source(&self, source: &PackageName) -> Option<Vec<&UDDBug>> {
        self.source_index
            .get(source)
            .map(|indices| indices.iter().map(|idx| &self.bugs[*idx]).collect())
    }
}

fn load_bugs_from_reader(reader: impl Read) -> Result<UDDBugs> {
    serde_yaml::from_reader(reader)
        .map_err(Into::into)
        .map(UDDBugs::new)
}

#[cfg(test)]
mod test {
    use super::{Severity, load_bugs_from_reader};

    const TEST_DATA: &str = r"
---
- id: 743062
  package: src:mutextrace
  source: mutextrace
  severity: serious
  title: 'mutextrace: sometimes FTBFS: testsuite races'
  last_modified: '2021-08-16'
  status: pending
  affects_stable: false
  affects_testing: false
  affects_unstable: true
  affects_experimental: false
  last_modified_full: '2021-08-16 07:03:39 +0000'
  autormdate: ''
- id: 778111
  package: src:scheme2c
  source: scheme2c
  severity: serious
  title: 'scheme2c: ftbfs with GCC-5'
  last_modified: '2021-08-16'
  status: pending
  affects_stable: false
  affects_testing: false
  affects_unstable: true
  affects_experimental: false
  last_modified_full: '2021-08-16 07:03:46 +0000'
  autormdate: ''
- id: 789292
  package: src:dmtcp
  source: dmtcp
  severity: serious
  title: 'dmtcp: FTBFS with glibc-2.21 and gcc-5'
  last_modified: '2021-11-16'
  status: forwarded
  affects_stable: false
  affects_testing: false
  affects_unstable: true
  affects_experimental: false
  last_modified_full: '2021-11-16 23:03:16 +0000'
  autormdate: ''
- id: 1114387
  package: src:slurm-wlm,src:pmix
  source: pmix,slurm-wlm
  severity: serious
  title: 'slurm-wlm: FTBFS: configure: error: unable to locate pmix installation'
  last_modified: '2025-10-27'
  status: pending
  affects_stable: false
  affects_testing: true
  affects_unstable: true
  affects_experimental: false
  last_modified_full: '2025-10-27 23:43:02 +0000'
  autormdate: ''
";

    #[test]
    fn read_bugs() {
        let bugs = load_bugs_from_reader(TEST_DATA.as_bytes()).unwrap();

        assert!(bugs.bugs_for_source(&"dmtcp".try_into().unwrap()).is_some());
        assert!(
            bugs.bugs_for_source(&"zathura".try_into().unwrap())
                .is_none()
        );

        for bug in bugs
            .bugs_for_source(&"mutextrace".try_into().unwrap())
            .unwrap()
        {
            assert!(bug.severity >= Severity::Serious);
            // assert!(bug.severity.is_rc());
        }
    }
}
