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

impl Architecture {
    /// Check if the architecture matches an architecture wildcard
    pub fn matches_wildcard(&self, at: ArchitectureTuple) -> bool {
        let converted = ArchitectureTuple::from(*self);
        at.contains(converted)
    }
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

/// Representation of ABIs known by `dpkg`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ABI {
    /// Generic base ABI
    Base,
    /// eabi for ARM
    EmbeddedABI,
    /// eabihf for ARM
    EmbeddedABIHardFloat,
    /// x32 ABI for x86-64
    X32,
    /// abi64 for mips64
    ABI64,
    /// abin32 for mips
    ABIn32,
}

impl Display for ABI {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

impl AsRef<str> for ABI {
    fn as_ref(&self) -> &str {
        match self {
            Self::Base => "base",
            Self::EmbeddedABI => "eabi",
            Self::EmbeddedABIHardFloat => "eabihf",
            Self::X32 => "x32",
            Self::ABI64 => "abi64",
            Self::ABIn32 => "abin32",
        }
    }
}

impl TryFrom<&str> for ABI {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "base" => Ok(Self::Base),
            "eabi" => Ok(Self::EmbeddedABI),
            "eabihf" => Ok(Self::EmbeddedABIHardFloat),
            "x32" => Ok(Self::X32),
            "abi64" => Ok(Self::ABI64),
            "abin32" => Ok(Self::ABIn32),
            _ => Err(ParseError::InvalidArchitecture),
        }
    }
}

/// Representation of libcs known by `dpkg`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LibC {
    /// GNU's `libc`, i.e., `glibc`
    GNU,
    /// `musl`
    Musl,
    /// `uclibc`
    UCLibC,
    /// BSD's `libc`
    BSD,
}

impl Display for LibC {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

impl AsRef<str> for LibC {
    fn as_ref(&self) -> &str {
        match self {
            Self::GNU => "gnu",
            Self::Musl => "musl",
            Self::UCLibC => "uclibc",
            Self::BSD => "bsd",
        }
    }
}

impl TryFrom<&str> for LibC {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "gnu" => Ok(Self::GNU),
            "musl" => Ok(Self::Musl),
            "uclibc" => Ok(Self::UCLibC),
            "bsd" => Ok(Self::BSD),
            _ => Err(ParseError::InvalidArchitecture),
        }
    }
}

/// Representation of operating systems known by `dpkg`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperatingSystem {
    /// Linux
    Linux,
    /// Hurd
    Hurd,
    /// FreeBSD
    FreeBSD,
    /// OpenBSD
    OpenBSD,
    /// NetBSD
    NetBSD,
    /// Darwin
    Darwin,
    /// AIX
    AIX,
    /// Solaris
    Solaris,
}

impl Display for OperatingSystem {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

impl AsRef<str> for OperatingSystem {
    fn as_ref(&self) -> &str {
        match self {
            Self::Linux => "linux",
            Self::Hurd => "hurd",
            Self::FreeBSD => "freebsd",
            Self::OpenBSD => "openbsd",
            Self::NetBSD => "netbsd",
            Self::Darwin => "darwin",
            Self::AIX => "aix",
            Self::Solaris => "solaris",
        }
    }
}

impl TryFrom<&str> for OperatingSystem {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "linux" => Ok(Self::Linux),
            "hurd" => Ok(Self::Hurd),
            "freebsd" => Ok(Self::FreeBSD),
            "openbsd" => Ok(Self::OpenBSD),
            "netbsd" => Ok(Self::NetBSD),
            "darwin" => Ok(Self::Darwin),
            "aix" => Ok(Self::AIX),
            "solaris" => Ok(Self::Solaris),
            _ => Err(ParseError::InvalidArchitecture),
        }
    }
}

/// Representation of CPUs known by `dpkg`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CPU {
    /// Alpha
    Alpha,
    /// x86-64
    Amd64,
    /// arc
    Arc,
    /// ARM big endian
    Armeb,
    /// ARM
    Arm,
    /// ARM64
    Arm64,
    /// hppa
    Hppa,
    /// loong64
    Loong64,
    /// i386
    I386,
    /// ia64
    Ia64,
    /// m86k
    M68k,
    /// MIPS
    Mips,
    /// MIPS little endian
    Mipsel,
    /// MIPS R6
    MipsR6,
    /// MIPS R6 little endian
    MipsR6el,
    /// MIPS64
    Mips64,
    /// MIPS64 little endian
    Mips64el,
    /// MIPS64 R6
    Mips64R6,
    /// MIPS64 R6 little endian
    Mips64R6el,
    /// nios2
    Nios2,
    /// or1k
    Or1k,
    /// PpwerPC
    PowerPC,
    /// PowerPC little endian
    PowerPCel,
    /// PowerPC 64
    Ppc64,
    /// PowerPC 64 little endian
    Ppc64el,
    /// RISC-V 64
    Riscv64,
    /// s390
    S390,
    /// s390x
    S390x,
    /// sh3
    Sh3,
    /// sh3 big endian
    Sh3eb,
    /// sh4
    Sh4,
    /// sh4 big endian
    Sh4eb,
    /// sparc
    Sparc,
    /// sparc 64
    Sparc64,
}

impl Display for CPU {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

impl AsRef<str> for CPU {
    fn as_ref(&self) -> &str {
        match self {
            Self::Alpha => "alpha",
            Self::Amd64 => "amd64",
            Self::Arc => "arc",
            Self::Armeb => "armeb",
            Self::Arm => "arm",
            Self::Arm64 => "arm64",
            Self::Hppa => "hppa",
            Self::Loong64 => "loong64",
            Self::I386 => "i386",
            Self::Ia64 => "ia64",
            Self::M68k => "m68k",
            Self::Mips => "mips",
            Self::Mipsel => "mipsel",
            Self::MipsR6 => "mipsr6",
            Self::MipsR6el => "mipsr6el",
            Self::Mips64 => "mips64",
            Self::Mips64el => "mips64el",
            Self::Mips64R6 => "mips64r6",
            Self::Mips64R6el => "mips64r6el",
            Self::Nios2 => "nios2",
            Self::Or1k => "or1k",
            Self::PowerPC => "powerpc",
            Self::PowerPCel => "powerpcel",
            Self::Ppc64 => "ppc64",
            Self::Ppc64el => "ppc64el",
            Self::Riscv64 => "riscv64",
            Self::S390 => "s390",
            Self::S390x => "s390x",
            Self::Sh3 => "sh3",
            Self::Sh3eb => "sh3eb",
            Self::Sh4 => "sh4",
            Self::Sh4eb => "sh4eb",
            Self::Sparc => "sparc",
            Self::Sparc64 => "sparc64",
        }
    }
}

impl TryFrom<&str> for CPU {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "alpha" => Ok(Self::Alpha),
            "amd64" => Ok(Self::Amd64),
            "arc" => Ok(Self::Arc),
            "armeb" => Ok(Self::Armeb),
            "arm" => Ok(Self::Arm),
            "arm64" => Ok(Self::Arm64),
            "hppa" => Ok(Self::Hppa),
            "loong64" => Ok(Self::Loong64),
            "i386" => Ok(Self::I386),
            "ia64" => Ok(Self::Ia64),
            "m68k" => Ok(Self::M68k),
            "mips" => Ok(Self::Mips),
            "mipsel" => Ok(Self::Mipsel),
            "mipsr6" => Ok(Self::MipsR6),
            "mipsr6el" => Ok(Self::MipsR6el),
            "mips64" => Ok(Self::Mips64),
            "mips64el" => Ok(Self::Mips64el),
            "mips64r6" => Ok(Self::Mips64R6),
            "mips64r6el" => Ok(Self::Mips64R6el),
            "nios2" => Ok(Self::Nios2),
            "or1k" => Ok(Self::Or1k),
            "powerpc" => Ok(Self::PowerPC),
            "powerpcel" => Ok(Self::PowerPCel),
            "ppc64" => Ok(Self::Ppc64),
            "ppc64el" => Ok(Self::Ppc64el),
            "riscv64" => Ok(Self::Riscv64),
            "s390" => Ok(Self::S390),
            "s390x" => Ok(Self::S390x),
            "sh3" => Ok(Self::Sh3),
            "sh3eb" => Ok(Self::Sh3eb),
            "sh4" => Ok(Self::Sh4),
            "sh4eb" => Ok(Self::Sh4eb),
            "sparc" => Ok(Self::Sparc),
            "sparc64" => Ok(Self::Sparc64),
            _ => Err(ParseError::InvalidArchitecture),
        }
    }
}

/// An architecture tuple consisting of an ABI, libc, OS, and CPU.
///
/// Any value can be `None` to denote `any`. Hence, the tuple also works as wildcard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ArchitectureTuple {
    abi: Option<ABI>,
    libc: Option<LibC>,
    os: Option<OperatingSystem>,
    cpu: Option<CPU>,
}

impl ArchitectureTuple {
    /// Checker whether an architecture wildcard contains another
    pub fn contains(&self, other: Self) -> bool {
        (self.abi.is_none() || other.abi == self.abi)
            && (self.libc.is_none() || other.libc == self.libc)
            && (self.os.is_none() || other.os == self.os)
            && (self.cpu.is_none() || other.cpu == self.cpu)
    }
}

impl Display for ArchitectureTuple {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(abi) = self.abi {
            write!(f, "{}-", abi)?;
        } else {
            write!(f, "any-")?;
        }
        if let Some(libc) = self.libc {
            write!(f, "{}-", libc)?;
        } else {
            write!(f, "any-")?;
        }
        if let Some(os) = self.os {
            write!(f, "{}-", os)?;
        } else {
            write!(f, "any-")?;
        }
        if let Some(cpu) = self.cpu {
            write!(f, "{}", cpu)
        } else {
            write!(f, "any")
        }
    }
}

impl From<Architecture> for ArchitectureTuple {
    fn from(value: Architecture) -> Self {
        match value {
            // there is not really a good fit for `all` and `source`
            Architecture::All | Architecture::Source => Self {
                abi: None,
                libc: None,
                os: None,
                cpu: None,
            },
            Architecture::Alpha => Self {
                abi: Some(ABI::Base),
                libc: Some(LibC::GNU),
                os: Some(OperatingSystem::Linux),
                cpu: Some(CPU::Alpha),
            },
            Architecture::Amd64 => Self {
                abi: Some(ABI::Base),
                libc: Some(LibC::GNU),
                os: Some(OperatingSystem::Linux),
                cpu: Some(CPU::Amd64),
            },
            Architecture::Arm64 => Self {
                abi: Some(ABI::Base),
                libc: Some(LibC::GNU),
                os: Some(OperatingSystem::Linux),
                cpu: Some(CPU::Arm64),
            },
            Architecture::Armel => Self {
                abi: Some(ABI::EmbeddedABI),
                libc: Some(LibC::GNU),
                os: Some(OperatingSystem::Linux),
                cpu: Some(CPU::Arm),
            },
            Architecture::Armhf => Self {
                abi: Some(ABI::EmbeddedABIHardFloat),
                libc: Some(LibC::GNU),
                os: Some(OperatingSystem::Linux),
                cpu: Some(CPU::Arm),
            },
            Architecture::Hppa => Self {
                abi: Some(ABI::Base),
                libc: Some(LibC::GNU),
                os: Some(OperatingSystem::Linux),
                cpu: Some(CPU::Hppa),
            },
            Architecture::HurdAmd64 => Self {
                abi: Some(ABI::Base),
                libc: Some(LibC::GNU),
                os: Some(OperatingSystem::Hurd),
                cpu: Some(CPU::Amd64),
            },
            Architecture::HurdI386 => Self {
                abi: Some(ABI::Base),
                libc: Some(LibC::GNU),
                os: Some(OperatingSystem::Hurd),
                cpu: Some(CPU::I386),
            },
            Architecture::I386 => Self {
                abi: Some(ABI::Base),
                libc: Some(LibC::GNU),
                os: Some(OperatingSystem::Linux),
                cpu: Some(CPU::I386),
            },
            Architecture::Ia64 => Self {
                abi: Some(ABI::Base),
                libc: Some(LibC::GNU),
                os: Some(OperatingSystem::Linux),
                cpu: Some(CPU::Ia64),
            },
            Architecture::Loong64 => Self {
                abi: Some(ABI::Base),
                libc: Some(LibC::GNU),
                os: Some(OperatingSystem::Linux),
                cpu: Some(CPU::Loong64),
            },
            Architecture::M68k => Self {
                abi: Some(ABI::Base),
                libc: Some(LibC::GNU),
                os: Some(OperatingSystem::Linux),
                cpu: Some(CPU::M68k),
            },
            Architecture::Mips64el => Self {
                abi: Some(ABI::ABI64),
                libc: Some(LibC::GNU),
                os: Some(OperatingSystem::Linux),
                cpu: Some(CPU::Mips64el),
            },
            Architecture::Mipsel => Self {
                abi: Some(ABI::ABIn32),
                libc: Some(LibC::GNU),
                os: Some(OperatingSystem::Linux),
                cpu: Some(CPU::Mips64el),
            },
            Architecture::PowerPC => Self {
                abi: Some(ABI::Base),
                libc: Some(LibC::GNU),
                os: Some(OperatingSystem::Linux),
                cpu: Some(CPU::PowerPC),
            },
            Architecture::Ppc64 => Self {
                abi: Some(ABI::Base),
                libc: Some(LibC::GNU),
                os: Some(OperatingSystem::Linux),
                cpu: Some(CPU::Ppc64),
            },
            Architecture::Ppc64el => Self {
                abi: Some(ABI::Base),
                libc: Some(LibC::GNU),
                os: Some(OperatingSystem::Linux),
                cpu: Some(CPU::Ppc64el),
            },
            Architecture::Riscv64 => Self {
                abi: Some(ABI::Base),
                libc: Some(LibC::GNU),
                os: Some(OperatingSystem::Linux),
                cpu: Some(CPU::Riscv64),
            },
            Architecture::S390x => Self {
                abi: Some(ABI::Base),
                libc: Some(LibC::GNU),
                os: Some(OperatingSystem::Linux),
                cpu: Some(CPU::S390x),
            },
            Architecture::Sh4 => Self {
                abi: Some(ABI::Base),
                libc: Some(LibC::GNU),
                os: Some(OperatingSystem::Linux),
                cpu: Some(CPU::S390x),
            },
            Architecture::Sparc64 => Self {
                abi: Some(ABI::Base),
                libc: Some(LibC::GNU),
                os: Some(OperatingSystem::Linux),
                cpu: Some(CPU::Sparc64),
            },
            Architecture::X32 => Self {
                abi: Some(ABI::X32),
                libc: Some(LibC::GNU),
                os: Some(OperatingSystem::Linux),
                cpu: Some(CPU::Amd64),
            },
        }
    }
}

impl TryFrom<&str> for ArchitectureTuple {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Err(ParseError::InvalidArchitecture);
        }

        let values: Vec<_> = value.split('-').collect();
        if values.len() > 4 {
            return Err(ParseError::InvalidArchitecture);
        }
        if !values.contains(&"any") && values.len() != 4 {
            // not a fully specified tuple and not a wildcard, so let's try it as an architectre
            return Architecture::try_from(value).map(Self::from);
        }

        // if it is wildcard, `any`s are pre-pended until there are four elements
        let values = match values.len() {
            1 => &["any", "any", "any", values[0]],
            2 => &["any", "any", values[0], values[1]],
            3 => &["any", values[0], values[1], values[2]],
            _ => values.as_slice(),
        };

        let abi = if values[0] != "any" {
            Some(ABI::try_from(values[0])?)
        } else {
            None
        };
        let libc = if values[1] != "any" {
            Some(LibC::try_from(values[1])?)
        } else {
            None
        };
        let os = if values[2] != "any" {
            Some(OperatingSystem::try_from(values[2])?)
        } else {
            None
        };
        let cpu = if values[3] != "any" {
            Some(CPU::try_from(values[3])?)
        } else {
            None
        };
        Ok(Self { abi, libc, os, cpu })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn from_str() {
        assert_eq!(
            Architecture::try_from("amd64").unwrap(),
            Architecture::Amd64
        );
    }

    #[test]
    fn matches_self() {
        assert!(Architecture::Amd64.matches_wildcard(Architecture::Amd64.into()));
    }

    #[test]
    fn tuple_from_str() {
        assert_eq!(
            ArchitectureTuple::try_from("amd64").unwrap(),
            Architecture::Amd64.into()
        );
        assert_eq!(
            ArchitectureTuple::try_from("hurd-amd64").unwrap(),
            Architecture::HurdAmd64.into()
        );
        assert_ne!(
            ArchitectureTuple::try_from("any").unwrap(),
            Architecture::Amd64.into()
        );
        assert_eq!(
            ArchitectureTuple::try_from("any").unwrap(),
            ArchitectureTuple::try_from("any-any-any-any").unwrap(),
        );
        assert_eq!(
            ArchitectureTuple::try_from("any").unwrap(),
            ArchitectureTuple::try_from("any-any-any").unwrap(),
        );
        assert_eq!(
            ArchitectureTuple::try_from("any").unwrap(),
            ArchitectureTuple::try_from("any-any").unwrap(),
        );
        assert_eq!(
            ArchitectureTuple::try_from("linux-any").unwrap(),
            ArchitectureTuple::try_from("any-any-linux-any").unwrap(),
        );
        assert_eq!(
            ArchitectureTuple::try_from("gnu-any-any").unwrap(),
            ArchitectureTuple::try_from("any-gnu-any-any").unwrap(),
        );

        let any_amd64 = ArchitectureTuple::try_from("any-amd64").unwrap();
        let linux_any = ArchitectureTuple::try_from("linux-any").unwrap();
        assert!(Architecture::Amd64.matches_wildcard(any_amd64));
        assert!(Architecture::Amd64.matches_wildcard(linux_any));
    }
}
