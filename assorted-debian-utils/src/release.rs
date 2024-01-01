// Copyright 2024 Sebastian Ramacher
// SPDX-License-Identifier: LGPL-3.0-or-later

//! # Helper to handle `Release` files`

use std::collections::HashMap;
use std::fmt::Formatter;
use std::io::{BufRead, Cursor};

use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::architectures::Architecture;
use crate::archive::{Codename, Component, Suite};
use crate::utils::{DateTimeVisitor, WhitespaceListVisitor};

/// Deserialize a datetime string into a `DateTime<Utc>`
fn deserialize_datetime<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserializer.deserialize_str(DateTimeVisitor("%a, %d %b %Y %H:%M:%S %Z"))
}

/// Deserialize a datetime string into a `Option<DateTime<Utc>>`
fn deserialize_datetime_option<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_datetime(deserializer).map(Some)
}

/// Deserialize a list of architectures into a `Vec<Architecture>`
fn deserialize_architectures<'de, D>(deserializer: D) -> Result<Vec<Architecture>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserializer.deserialize_str(WhitespaceListVisitor::<Architecture>::new())
}

/// Deserialize a list of components into a `Vec<Component>`
fn deserialize_components<'de, D>(deserializer: D) -> Result<Vec<Component>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserializer.deserialize_str(WhitespaceListVisitor::<Component>::new())
}

#[derive(Debug)]
struct SHA256Visitor;

impl<'de> serde::de::Visitor<'de> for SHA256Visitor {
    type Value = HashMap<String, FileInfo>;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        write!(formatter, "a list of files")
    }

    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let cursor = Cursor::new(s);
        // cursor.lines().filter_map(|line| if let Ok(line) = line { line.split_whitespace()} )
        let mut ret: HashMap<String, FileInfo> = Default::default();
        for line in cursor.lines() {
            let Ok(line) = line else {
                break;
            };

            let fields: Vec<_> = line.split_ascii_whitespace().collect();
            if fields.len() != 3 {
                return Err(E::invalid_value(serde::de::Unexpected::Str(&line), &self));
            }

            let file = fields[2];
            let file_size = fields[1].parse().map_err(E::custom)?;
            let hash = hex::decode(fields[0]).map_err(E::custom)?;

            ret.insert(
                file.to_string(),
                FileInfo {
                    file_size,
                    hash: hash.try_into().map_err(|_| {
                        E::invalid_value(serde::de::Unexpected::Str(fields[0]), &self)
                    })?,
                },
            );
        }
        Ok(ret)
    }
}

/// Deserialize files listed as SHA256
fn deserialize_sha256<'de, D>(deserializer: D) -> Result<HashMap<String, FileInfo>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserializer.deserialize_str(SHA256Visitor)
}

/// Representation of a `Release` file`
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct FileInfo {
    file_size: u64,
    hash: [u8; 32],
}

/// Representation of a `Release` file`
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub struct Release {
    /// Origin of the release
    pub origin: String,
    /// Label of the release
    pub label: String,
    /// Suite of the release
    pub suite: Suite,
    /// Suite of the release
    pub codename: Codename,
    /// Version of the release
    pub version: Option<String>,
    /// Date of the release
    #[serde(deserialize_with = "deserialize_datetime")]
    pub date: DateTime<Utc>,
    #[serde(
        default,
        deserialize_with = "deserialize_datetime_option",
        rename = "Valid-Until"
    )]
    /// Validity of the release
    pub valid_until: Option<DateTime<Utc>>,
    #[serde(rename = "Acquire-by-Hash")]
    /// Whether files should be acquired by hash
    pub acquire_by_hash: Option<bool>,
    /// Supported architectures of the release
    #[serde(deserialize_with = "deserialize_architectures")]
    pub architectures: Vec<Architecture>,
    /// Components of the release
    #[serde(deserialize_with = "deserialize_components")]
    pub components: Vec<Component>,
    /// Release description
    pub description: String,
    /// Referenced `Package` files and others from the release
    #[serde(rename = "SHA256", deserialize_with = "deserialize_sha256")]
    pub files: HashMap<String, FileInfo>,
}

/// Read release from a reader
pub fn from_reader(reader: impl BufRead) -> Result<Release, rfc822_like::de::Error> {
    rfc822_like::from_reader(reader)
}

/// Read release from a string
pub fn from_str(data: &str) -> Result<Release, rfc822_like::de::Error> {
    rfc822_like::from_str(data)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn archive() {
        let data = r#"Origin: Debian-ramacher.at
Label: Debian-ramacher.at
Suite: unstable
Codename: sid
Version: 13.0
Date: Sun, 17 Dec 2023 18:43:37 UTC
Architectures: i386 amd64
Components: main
Description: Experimental and unfinished Debian packages (for unstable)
MD5Sum:
 628a4efab35e598c7b6debdb0ac85314 26187 main/binary-i386/Packages
 6c849211e65839aac2682c461c82dbb3 7777 main/binary-i386/Packages.gz
 05ee2bfa660c3acc3559928769c29730 191 main/binary-i386/Release
 d41d8cd98f00b204e9800998ecf8427e 0 main/debian-installer/binary-i386/Packages
 7029066c27ac6f5ef18d660d5741979a 20 main/debian-installer/binary-i386/Packages.gz
 296265926c83b0d9d9d43fcc6c43496d 30187 main/binary-amd64/Packages
 8dad6d33daa175a4a54b9d328e9bb491 8821 main/binary-amd64/Packages.gz
 c0f8f3dd5202483a2b57bb348a3741a6 192 main/binary-amd64/Release
 d41d8cd98f00b204e9800998ecf8427e 0 main/debian-installer/binary-amd64/Packages
 7029066c27ac6f5ef18d660d5741979a 20 main/debian-installer/binary-amd64/Packages.gz
 4b35b2727e9c1d87c775e35fd8d00cf4 15130 main/source/Sources
 689c40d665e43a8f9a94d6e2b1dd47a4 4582 main/source/Sources.gz
 3ce12e6e384a34e6e1850bcc192edf8c 193 main/source/Release
SHA1:
 da7a5b4f20e79cab9bacca996d83419d5224a709 26187 main/binary-i386/Packages
 a0b5ae4166358c741f1c27bf457c3b31bcdb495a 7777 main/binary-i386/Packages.gz
 046a2ee510a7ea14c8b718dd153077b0359b3509 191 main/binary-i386/Release
 da39a3ee5e6b4b0d3255bfef95601890afd80709 0 main/debian-installer/binary-i386/Packages
 46c6643f07aa7f6bfe7118de926b86defc5087c4 20 main/debian-installer/binary-i386/Packages.gz
 d7fc79844dbc2702ca889a985f716374f7c8b9a5 30187 main/binary-amd64/Packages
 21374a60ce3d47b87bac11b3b3a96795020a0d41 8821 main/binary-amd64/Packages.gz
 01f970b6eae435dd8b6b1f8f61727db854212ce4 192 main/binary-amd64/Release
 da39a3ee5e6b4b0d3255bfef95601890afd80709 0 main/debian-installer/binary-amd64/Packages
 46c6643f07aa7f6bfe7118de926b86defc5087c4 20 main/debian-installer/binary-amd64/Packages.gz
 12b46a55c05518bfcfb267908185f041a1b984ae 15130 main/source/Sources
 5e2bfa609cbc328e07336f8e17707683fda37011 4582 main/source/Sources.gz
 96d0688be60481ba7eb71007b609bdf1f8323725 193 main/source/Release
SHA256:
 efe2dafdf6a50f376af1dfc574d6bd3360558fde917555671b13832c89604d9f 26187 main/binary-i386/Packages
 ba66d22607be572323b72ca152d6e635fab075d92a2265bbfe319337c35ccd13 7777 main/binary-i386/Packages.gz
 e6be53e3210056ed6854cf2a362cb953eaa962ea811cfbe34cdad2807be61101 191 main/binary-i386/Release
 e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 0 main/debian-installer/binary-i386/Packages
 59869db34853933b239f1e2219cf7d431da006aa919635478511fabbfc8849d2 20 main/debian-installer/binary-i386/Packages.gz
 baf930986b322ef7ff8cc04fa57762c68e7f9d8b67a0423bd5441686cbf3e751 30187 main/binary-amd64/Packages
 0ad7ab0202ece24b57051f16010c72479b97e905c659f975eac5d69284c562f3 8821 main/binary-amd64/Packages.gz
 97e06eefea86617e4abc8a647d0faebd0eaca7c87031423a4ae1d38e8f1c97bb 192 main/binary-amd64/Release
 e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 0 main/debian-installer/binary-amd64/Packages
 59869db34853933b239f1e2219cf7d431da006aa919635478511fabbfc8849d2 20 main/debian-installer/binary-amd64/Packages.gz
 b0a524d1ba90e253c937859e3ce30bc49a291e33dbb8124706424cf5c06100a8 15130 main/source/Sources
 2bc04b364bfc30657836faf8d1de7f6044652bcca6af6503ef404a086897267a 4582 main/source/Sources.gz
 3637559f78ac17d0e55bce465d510ef912d539e4b810a66b32431dd76f5929d8 193 main/source/Release"#;
        let release = from_str(data).unwrap();

        assert_eq!(
            release.architectures,
            vec![Architecture::I386, Architecture::Amd64]
        );
        assert_eq!(release.components, vec![Component::Main]);
        assert_eq!(release.suite, Suite::Unstable);
        assert_eq!(release.codename, Codename::Sid);
        assert!(release.files.contains_key("main/source/Release"));
        assert_eq!(
            release.files["main/source/Release"],
            FileInfo {
                file_size: 193,
                hash: [
                    0x36, 0x37, 0x55, 0x9f, 0x78, 0xac, 0x17, 0xd0, 0xe5, 0x5b, 0xce, 0x46, 0x5d,
                    0x51, 0x0e, 0xf9, 0x12, 0xd5, 0x39, 0xe4, 0xb8, 0x10, 0xa6, 0x6b, 0x32, 0x43,
                    0x1d, 0xd7, 0x6f, 0x59, 0x29, 0xd8
                ]
            }
        );
    }
}
