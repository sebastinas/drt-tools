// Copyright 2022 Sebastian Ramacher
// SPDX-License-Identifier: LGPL-3.0-or-later

//! # Helpers to handle `autoremovals.yaml`
//!
//! This module provides helpers to deserialize [autoremovals.yaml](https://udd.debian.org/cgi-bin/autoremovals.yaml.cgi)
//! with [serde].

use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::{collections::HashMap, io};

use crate::{utils::DateTimeVisitor, version::PackageVersion};

/// Deserialize a datetime string into a `DateTime<Utc>`
fn deserialize_datetime<'de, D>(deserializer: D) -> std::result::Result<DateTime<Utc>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserializer.deserialize_str(DateTimeVisitor("%Y-%m-%d %H:%M:%S"))
}

/// All autoremovals
pub type AutoRemovals = HashMap<String, AutoRemoval>;

/// An autoremoval
#[derive(Debug, Deserialize, PartialEq)]
pub struct AutoRemoval {
    /// The package's RC bugs causing auto-removal.
    pub bugs: Vec<String>,
    /// The RC bugs of dependencies causing auto-removal.
    pub bugs_dependencies: Option<Vec<String>>,
    /// List of RC-buggy dependencies causing auto-removal.
    pub buggy_dependencies: Option<Vec<String>>,
    /// Auto-removal is caused by dependencies.
    pub dependencies_only: bool,
    /// Date of the last check.
    #[serde(deserialize_with = "deserialize_datetime")]
    pub last_checked: DateTime<Utc>,
    /// Auto-removal date.
    #[serde(deserialize_with = "deserialize_datetime")]
    pub removal_date: DateTime<Utc>,
    /// Source package name.
    pub source: String,
    /// Package version.
    pub version: PackageVersion,
    /// List of reverse dependencies that will also be auto-removed.
    pub rdeps: Option<Vec<String>>,
}

/// Result type
pub type Result<T> = serde_yaml::Result<T>;

/// Read autoremovals from a reader
pub fn from_reader(reader: impl io::Read) -> Result<AutoRemovals> {
    serde_yaml::from_reader(reader)
}

/// Read autoremovals from a string
pub fn from_str(data: &str) -> Result<AutoRemovals> {
    serde_yaml::from_str(data)
}

#[cfg(test)]
mod test {
    use super::from_str;

    #[test]
    fn base() {
        let data = r#"---
mplayer:
  bugs:
  - '1005899'
  dependencies_only: false
  last_checked: 2022-04-10 17:55:40
  rdeps:
  - devede
  - diffoscope
  - dradio
  - mplayer-blue
  - ogmrip
  - qwinff
  - reprotest
  - vdr-plugin-mp3
  - videotrans
  removal_date: 2022-05-01 19:42:01
  source: mplayer
  version: 2:1.4+ds1-3
mplayer-blue:
  buggy_dependencies:
  - mplayer
  bugs: []
  bugs_dependencies:
  - '1005899'
  dependencies_only: true
  last_checked: 2022-04-10 17:55:40
  removal_date: 2022-05-01 19:42:01
  source: mplayer-blue
  version: 1.13-2
"#;
        let autoremovals = from_str(data).unwrap();

        assert!(autoremovals.contains_key("mplayer"));
        let mplayer = autoremovals.get("mplayer").unwrap();
        assert_eq!(mplayer.source, "mplayer");
        assert_eq!(mplayer.version.to_string(), "2:1.4+ds1-3");
        assert_eq!(mplayer.bugs, vec!["1005899"]);
        assert_eq!(
            mplayer.rdeps.as_ref().unwrap(),
            &vec![
                "devede",
                "diffoscope",
                "dradio",
                "mplayer-blue",
                "ogmrip",
                "qwinff",
                "reprotest",
                "vdr-plugin-mp3",
                "videotrans"
            ]
        );
        assert!(!mplayer.dependencies_only);

        assert!(autoremovals.contains_key("mplayer-blue"));
        let mplayer_blue = autoremovals.get("mplayer-blue").unwrap();
        assert_eq!(mplayer_blue.source, "mplayer-blue");
        assert_eq!(mplayer_blue.version.to_string(), "1.13-2");
        assert_eq!(mplayer_blue.bugs.len(), 0);
        assert_eq!(
            mplayer_blue.buggy_dependencies.as_ref().unwrap(),
            &vec!["mplayer"]
        );
        assert_eq!(
            mplayer_blue.bugs_dependencies.as_ref().unwrap(),
            &vec!["1005899"]
        );
        assert!(mplayer_blue.dependencies_only);
    }
}
