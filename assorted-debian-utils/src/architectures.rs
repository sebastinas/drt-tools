// Copyright 2021 Sebastian Ramacher
// SPDX-License-Identifier: LGPL-3.0-or-later

//! # Helpers to handle Debian architectures
//!
//! This module provides helpers for Debian architectures. This currently involves a list of release
//! architectures and an enum for release and ports architectures.

use serde::Deserialize;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

/// Debian architectures
///
/// This enum describes architectures that are release architectures or available on Debian ports.
/// It also provides `All` as special case for binary independent packages.
#[derive(Clone, Debug, Deserialize, PartialEq, Hash, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Architecture {
    /// The `all` architecture for architecture independent packages
    All,
    /// The `alpha` architecture
    Alpha,
    /// The `amd64` architecture
    Amd64,
    /// The `arm64` architecture
    Arm64,
    /// The `armel` architecture
    Armel,
    /// The `armhf` architecture
    Armhf,
    /// The `hppa` architecture
    Hppa,
    /// The `hurd-i386` architecture
    #[serde(rename = "hurd-i386")]
    HurdI386,
    /// The `i386` architecture
    I386,
    /// The `ia64` architecture
    Ia64,
    /// The `kfreebsd-amd64` architecture
    #[serde(rename = "kfreebsd-amd64")]
    KFreeBSDAmd64,
    /// The `kfreebsd-i386` architecture
    #[serde(rename = "kfreebsd-i386")]
    KFreeBSDI386,
    /// The `m86k` architecture
    M86k,
    /// The `mips64el` architecture
    Mips64el,
    /// The `mipsel` architecture
    Mipsel,
    /// The `powerpc` architecture
    PowerPC,
    /// The `ppc64` architecture
    Ppc64,
    /// The `ppc64el` architecture
    Ppc64el,
    /// The `riscv64` architecture
    Riscv64,
    /// The `s390x` architecture
    S390x,
    /// The `sh4` architecture
    Sh4,
    /// The `sparc64` architecture
    Sparc64,
    /// The `x32` architecture
    X32,
}

impl Display for Architecture {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Architecture::All => "all",
                Architecture::Alpha => "alpha",
                Architecture::Amd64 => "amd64",
                Architecture::Arm64 => "arm64",
                Architecture::Armel => "armel",
                Architecture::Armhf => "armhf",
                Architecture::Hppa => "hppa",
                Architecture::HurdI386 => "hurd-i386",
                Architecture::I386 => "i386",
                Architecture::Ia64 => "ia64",
                Architecture::KFreeBSDAmd64 => "kfreebsd-amd64",
                Architecture::KFreeBSDI386 => "kfreebsd-i386",
                Architecture::M86k => "m86k",
                Architecture::Mips64el => "mips64el",
                Architecture::Mipsel => "mipsel",
                Architecture::PowerPC => "powerpc",
                Architecture::Ppc64 => "ppc64",
                Architecture::Ppc64el => "ppc64el",
                Architecture::Riscv64 => "risc64",
                Architecture::S390x => "s390x",
                Architecture::Sh4 => "sh4",
                Architecture::Sparc64 => "sparc64",
                Architecture::X32 => "x32",
            }
        )
    }
}

/// Parsing of an architecture failed
pub enum ParseError {
    /// Given string is not a valid architecture
    InvalidArchitecture,
}

impl TryFrom<&str> for Architecture {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "all" => Ok(Architecture::All),
            "alpha" => Ok(Architecture::Alpha),
            "amd64" => Ok(Architecture::Amd64),
            "arm64" => Ok(Architecture::Arm64),
            "armel" => Ok(Architecture::Armel),
            "armhf" => Ok(Architecture::Armhf),
            "hppa" => Ok(Architecture::Hppa),
            "hurd-i386" => Ok(Architecture::HurdI386),
            "i386" => Ok(Architecture::I386),
            "ia64" => Ok(Architecture::Ia64),
            "kfreebsd-amd64" => Ok(Architecture::KFreeBSDAmd64),
            "kfreebsd-i386" => Ok(Architecture::KFreeBSDI386),
            "m86k" => Ok(Architecture::M86k),
            "mips64el" => Ok(Architecture::Mips64el),
            "mipsel" => Ok(Architecture::Mipsel),
            "powerpc" => Ok(Architecture::PowerPC),
            "ppc64" => Ok(Architecture::Ppc64),
            "ppc64el" => Ok(Architecture::Ppc64el),
            "risc64" => Ok(Architecture::Riscv64),
            "s390x" => Ok(Architecture::S390x),
            "sh4" => Ok(Architecture::Sh4),
            "sparc64" => Ok(Architecture::Sparc64),
            "x32" => Ok(Architecture::X32),
            _ => Err(ParseError::InvalidArchitecture),
        }
    }
}

impl FromStr for Architecture {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Architecture::try_from(s)
    }
}

/// Release architectures for bookworm
pub const RELEASE_ARCHITECTURES: [Architecture; 9] = [
    Architecture::Amd64,
    Architecture::Arm64,
    Architecture::Armel,
    Architecture::Armhf,
    Architecture::I386,
    Architecture::Ppc64el,
    Architecture::Mipsel,
    Architecture::Mips64el,
    Architecture::S390x,
];
