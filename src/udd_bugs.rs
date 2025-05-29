// Copyright 2022-2023 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
    io::Read,
};

use anyhow::Result;
use assorted_debian_utils::package::PackageName;
use serde::Deserialize;

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

#[derive(Clone, Debug, Deserialize)]
pub struct UDDBug {
    pub id: u32,
    pub source: PackageName,
    pub severity: Severity,
    pub title: String,
}

#[derive(Default)]
pub struct UDDBugs {
    bugs: Vec<UDDBug>,
    source_index: HashMap<PackageName, Vec<usize>>,
}

impl UDDBugs {
    pub fn new(bugs: Vec<UDDBug>) -> Self {
        let mut udd_bugs = Self {
            bugs,
            ..Default::default()
        };

        for (idx, bug) in udd_bugs.bugs.iter().enumerate() {
            udd_bugs
                .source_index
                .entry(bug.source.clone())
                .or_default()
                .push(idx);
        }

        udd_bugs
    }

    pub fn bugs_for_source(&self, source: &PackageName) -> Option<Vec<&UDDBug>> {
        self.source_index
            .get(source)
            .map(|indices| indices.iter().map(|idx| &self.bugs[*idx]).collect())
    }
}

pub fn load_bugs_from_reader(reader: impl Read) -> Result<UDDBugs> {
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
