// Copyright 2021 Sebastian Ramacher
// SPDX-License-Identifier: LGPL-3.0-or-later

//! # Collection of various utilities for Debian work
//!
//! This crate consists of the following modules:
//! * [architectures]: Helpers to handle Debian architectures
//! * [excuses]: Helpers to handle `excuses.yaml` for testing migration
//! * [wb]: Helpers to generate commands for wanna-build

#![warn(missing_docs)]
pub mod architectures;
pub mod excuses;
pub mod wb;
