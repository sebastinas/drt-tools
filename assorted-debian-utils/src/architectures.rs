// Copyright 2021 Sebastian Ramacher
// SPDX-License-Identifier: LGPL-3.0-or-later

//! # Helpers to handle Debian architectures
//!
//! This module provides helpers for Debian architectures. This currently involves a list of release
//! architectures and an enum for release and ports architectures.

use std::fmt::{Display, Formatter};
use std::str::FromStr;

use serde::{Deserialize, Serialize};

pub use crate::ParseError;

/// Debian architectures
///
/// This enum describes architectures that are release architectures or available on Debian ports.
/// It also provides `All` as special case for binary independent packages.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Hash, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Architecture {
    /// The `all` architecture for architecture independent packages
    All,
    /// The `alpha` architecture
    Alpha,
    /// The `amd64` architecture
    Amd64,
    /// The `arc` architecture
    Arc,
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
    /// The `m68k` architecture
    M68k,
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
    /// The `source` architecture
    Source,
}

impl Display for Architecture {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

impl AsRef<str> for Architecture {
    fn as_ref(&self) -> &str {
        match self {
            Architecture::All => "all",
            Architecture::Alpha => "alpha",
            Architecture::Amd64 => "amd64",
            Architecture::Arc => "arc",
            Architecture::Arm64 => "arm64",
            Architecture::Armel => "armel",
            Architecture::Armhf => "armhf",
            Architecture::Hppa => "hppa",
            Architecture::HurdI386 => "hurd-i386",
            Architecture::I386 => "i386",
            Architecture::Ia64 => "ia64",
            Architecture::M68k => "m68k",
            Architecture::Mips64el => "mips64el",
            Architecture::Mipsel => "mipsel",
            Architecture::PowerPC => "powerpc",
            Architecture::Ppc64 => "ppc64",
            Architecture::Ppc64el => "ppc64el",
            Architecture::Riscv64 => "riscv64",
            Architecture::S390x => "s390x",
            Architecture::Sh4 => "sh4",
            Architecture::Sparc64 => "sparc64",
            Architecture::X32 => "x32",
            Architecture::Source => "source",
        }
    }
}

impl TryFrom<&str> for Architecture {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "all" => Ok(Architecture::All),
            "alpha" => Ok(Architecture::Alpha),
            "amd64" => Ok(Architecture::Amd64),
            "arc" => Ok(Architecture::Arc),
            "arm64" => Ok(Architecture::Arm64),
            "armel" => Ok(Architecture::Armel),
            "armhf" => Ok(Architecture::Armhf),
            "hppa" => Ok(Architecture::Hppa),
            "hurd-i386" => Ok(Architecture::HurdI386),
            "i386" => Ok(Architecture::I386),
            "ia64" => Ok(Architecture::Ia64),
            "m68k" => Ok(Architecture::M68k),
            "mips64el" => Ok(Architecture::Mips64el),
            "mipsel" => Ok(Architecture::Mipsel),
            "powerpc" => Ok(Architecture::PowerPC),
            "ppc64" => Ok(Architecture::Ppc64),
            "ppc64el" => Ok(Architecture::Ppc64el),
            "riscv64" => Ok(Architecture::Riscv64),
            "s390x" => Ok(Architecture::S390x),
            "sh4" => Ok(Architecture::Sh4),
            "sparc64" => Ok(Architecture::Sparc64),
            "x32" => Ok(Architecture::X32),
            "source" => Ok(Architecture::Source),
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

/// Release architectures for trixie
pub const RELEASE_ARCHITECTURES: [Architecture; 8] = [
    Architecture::Amd64,
    Architecture::Arm64,
    Architecture::Armel,
    Architecture::Armhf,
    Architecture::I386,
    Architecture::Ppc64el,
    Architecture::Mips64el,
    Architecture::S390x,
];

/// Architectures in the Debian archive (unstable)
pub const ARCHIVE_ARCHITECTURES: [Architecture; 10] = [
    Architecture::Amd64,
    Architecture::Arm64,
    Architecture::Armel,
    Architecture::Armhf,
    Architecture::I386,
    Architecture::Ppc64el,
    Architecture::Mipsel,
    Architecture::Mips64el,
    Architecture::Riscv64,
    Architecture::S390x,
];

#[cfg(test)]
mod test {
    use super::Architecture;

    #[test]
    fn from_str() {
        assert_eq!(
            Architecture::try_from("amd64").unwrap(),
            Architecture::Amd64
        );
    }
}
