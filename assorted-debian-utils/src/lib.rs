// Copyright 2021 Sebastian Ramacher
// SPDX-License-Identifier: LGPL-3.0-or-later

//! # Collection of various utilities for Debian work
//!
//! This crate consists of the following modules:
//! * [architectures]: Helpers to handle Debian architectures
//! * [buildinfo]: Helpers to handle `.buildinfo` files
//! * [excuses]: Helpers to handle `excuses.yaml` for testing migration
//! * [wb]: Helpers to generate commands for wanna-build

#![warn(missing_docs)]

use std::{
    error::Error,
    fmt::{Display, Formatter},
};

pub mod architectures;
pub mod buildinfo;
pub mod excuses;
pub mod version;
pub mod wb;

#[cfg(feature = "libdpkg-sys")]
mod cversion;

/// Parsing error
#[derive(Debug)]
pub enum ParseError {
    /// Given string is not a valid architecture
    InvalidArchitecture,
    /// Given string is not a valid version
    InvalidVersion(version::VersionError),
}

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::InvalidArchitecture => write!(f, "invalid architecture"),
            ParseError::InvalidVersion(version_error) => {
                write!(f, "invalid version: {}", version_error)
            }
        }
    }
}

impl Error for ParseError {}
