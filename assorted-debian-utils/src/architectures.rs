use serde::Deserialize;
use std::fmt::{Display, Formatter};

/// A Debian architecture
#[derive(Clone, Debug, Deserialize, PartialEq, Hash, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Architecture {
    All,
    Alpha,
    Amd64,
    Arm64,
    Armel,
    Armhf,
    Hppa,
    #[serde(rename = "hurd-i386")]
    HurdI386,
    I386,
    Ia64,
    #[serde(rename = "kfreebsd-amd64")]
    KFreeBSDAmd64,
    #[serde(rename = "kfreebsd-i386")]
    KFreeBSDI386,
    M86k,
    Mips64el,
    Mipsel,
    PowerPC,
    Ppc64,
    Ppc64el,
    Riscv64,
    S390x,
    Sh4,
    Sparc64,
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
