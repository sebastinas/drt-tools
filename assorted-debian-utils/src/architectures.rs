// Copyright 2021-2024 Sebastian Ramacher
// SPDX-License-Identifier: LGPL-3.0-or-later

//! # Helpers to handle Debian architectures
//!
//! This module provides helpers for working with Debian architectures as they
//! appear in various files related to Debian binary and source packages,
//! archive indices, etc.

use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

use serde::{Deserialize, Serialize, de::Deserializer};

pub use crate::ParseError;
use crate::utils::WhitespaceListVisitor;

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
    /// The `arm64` architecture
    Arm64,
    /// The `armel` architecture
    Armel,
    /// The `armhf` architecture
    Armhf,
    /// The `hppa` architecture
    Hppa,
    /// The `hurd-amd64` architecture
    #[serde(rename = "hurd-amd64")]
    HurdAmd64,
    /// The `hurd-i386` architecture
    #[serde(rename = "hurd-i386")]
    HurdI386,
    /// The `i386` architecture
    I386,
    /// The `ia64` architecture
    Ia64,
    /// The `loong64` architecture
    Loong64,
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
            Self::All => "all",
            Self::Alpha => "alpha",
            Self::Amd64 => "amd64",
            Self::Arm64 => "arm64",
            Self::Armel => "armel",
            Self::Armhf => "armhf",
            Self::Hppa => "hppa",
            Self::HurdAmd64 => "hurd-amd64",
            Self::HurdI386 => "hurd-i386",
            Self::I386 => "i386",
            Self::Ia64 => "ia64",
            Self::Loong64 => "loong64",
            Self::M68k => "m68k",
            Self::Mips64el => "mips64el",
            Self::Mipsel => "mipsel",
            Self::PowerPC => "powerpc",
            Self::Ppc64 => "ppc64",
            Self::Ppc64el => "ppc64el",
            Self::Riscv64 => "riscv64",
            Self::S390x => "s390x",
            Self::Sh4 => "sh4",
            Self::Sparc64 => "sparc64",
            Self::X32 => "x32",
            Self::Source => "source",
        }
    }
}

impl TryFrom<&str> for Architecture {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "all" => Ok(Self::All),
            "alpha" => Ok(Self::Alpha),
            "amd64" => Ok(Self::Amd64),
            "arm64" => Ok(Self::Arm64),
            "armel" => Ok(Self::Armel),
            "armhf" => Ok(Self::Armhf),
            "hppa" => Ok(Self::Hppa),
            "hurd-amd64" => Ok(Self::HurdAmd64),
            "hurd-i386" => Ok(Self::HurdI386),
            "i386" => Ok(Self::I386),
            "ia64" => Ok(Self::Ia64),
            "loong64" => Ok(Self::Loong64),
            "m68k" => Ok(Self::M68k),
            "mips64el" => Ok(Self::Mips64el),
            "mipsel" => Ok(Self::Mipsel),
            "powerpc" => Ok(Self::PowerPC),
            "ppc64" => Ok(Self::Ppc64),
            "ppc64el" => Ok(Self::Ppc64el),
            "riscv64" => Ok(Self::Riscv64),
            "s390x" => Ok(Self::S390x),
            "sh4" => Ok(Self::Sh4),
            "sparc64" => Ok(Self::Sparc64),
            "x32" => Ok(Self::X32),
            "source" => Ok(Self::Source),
            _ => Err(ParseError::InvalidArchitecture),
        }
    }
}

impl FromStr for Architecture {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s)
    }
}

/// Deserialize a list of architectures into a `Vec<Architecture>`
pub(crate) fn deserialize_architectures<'de, D>(
    deserializer: D,
) -> Result<Vec<Architecture>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_str(WhitespaceListVisitor::new("Architecture"))
}

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
