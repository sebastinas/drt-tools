// Copyright 2021 Sebastian Ramacher
// SPDX-License-Identifier: LGPL-3.0-or-later

//! # Collection of various utilities for Debian work
//!
//! This crate consists of the following modules:
//! * [architectures]: Helpers to handle Debian architectures
//! * [archive]: Helpers for various features of the Debian archive
//! * [buildinfo]: Helpers to handle `.buildinfo` files
//! * [excuses]: Helpers to handle `excuses.yaml` for testing migration
//! * [version]: Helpers to handle package versions
//! * [wb]: Helpers to generate commands for wanna-build

#![warn(missing_docs)]
#![warn(missing_debug_implementations)]

use std::{
    error::Error,
    fmt::{Display, Formatter},
};

pub mod architectures;
pub mod archive;
pub mod autoremovals;
pub mod buildinfo;
pub mod excuses;
mod utils;
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
    /// Given string is not a valid suite
    InvalidSuite,
    /// Given string is not a valid suite or codename extension
    InvalidExtension,
    /// Given string is not a valid codename
    InvalidCodename,
    /// Given string is not a valid suite or codename
    InvalidSuiteOrCodename,
    /// Given string is not a valid multi-arch value
    InvalidMultiArch,
}

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::InvalidArchitecture => write!(f, "invalid architecture"),
            ParseError::InvalidVersion(version_error) => {
                write!(f, "invalid version: {}", version_error)
            }
            ParseError::InvalidSuite => write!(f, "invalid suite"),
            ParseError::InvalidExtension => write!(f, "invalid extension"),
            ParseError::InvalidCodename => write!(f, "invalid codename"),
            ParseError::InvalidSuiteOrCodename => write!(f, "invalid suite or codename"),
            ParseError::InvalidMultiArch => write!(f, "invalid multi-arch"),
        }
    }
}

impl Error for ParseError {}
