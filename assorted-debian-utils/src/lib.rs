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
#![warn(clippy::use_self)]

use thiserror::Error;

pub mod architectures;
pub mod archive;
pub mod autoremovals;
pub mod buildinfo;
pub mod excuses;
pub mod release;
mod utils;
pub mod version;
pub mod wb;

// Re-export rfc822_like
pub use rfc822_like;

/// Parsing error
#[derive(Clone, Copy, Debug, Error)]
pub enum ParseError {
    #[error("invalid architecture")]
    /// Given string is not a valid architecture
    InvalidArchitecture,
    #[error("invalid version: {0}")]
    /// Given string is not a valid version
    InvalidVersion(#[from] version::VersionError),
    #[error("invalid suite")]
    /// Given string is not a valid suite
    InvalidSuite,
    #[error("invalid extension")]
    /// Given string is not a valid suite or codename extension
    InvalidExtension,
    #[error("invalid codename")]
    /// Given string is not a valid codename
    InvalidCodename,
    #[error("invalid suite or codename")]
    /// Given string is not a valid suite or codename
    InvalidSuiteOrCodename,
    #[error("invalid multi-arch")]
    /// Given string is not a valid multi-arch value
    InvalidMultiArch,
    #[error("invalid component")]
    /// Given string is not a valid component
    InvalidComponent,
}
