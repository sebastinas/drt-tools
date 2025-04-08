// Copyright 2022 Sebastian Ramacher
// SPDX-License-Identifier: LGPL-3.0-or-later

//! # Representation of buildinfos
//!
//! This module provides `Buildinfo` to represent some fields of a `.buildinfo` file.

use std::io::BufRead;

use serde::{Deserialize, Deserializer};

use crate::{
    architectures::Architecture, package::PackageName, utils::WhitespaceListVisitor,
    version::PackageVersion,
};

fn deserialize_architecture<'de, D>(deserializer: D) -> Result<Vec<Architecture>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_str(WhitespaceListVisitor::<Architecture>::new())
}

/// A build info
#[derive(Debug, PartialEq, Eq, Deserialize, Hash)]
#[serde(rename_all = "PascalCase")]
pub struct Buildinfo {
    /// Source package
    pub source: PackageName,
    /// Version of the package
    pub version: PackageVersion,
    /// Architectures of the build
    #[serde(deserialize_with = "deserialize_architecture")]
    pub architecture: Vec<Architecture>,
}

/// Read buildinfo from a reader
pub fn from_reader(reader: impl BufRead) -> Result<Buildinfo, rfc822_like::de::Error> {
    rfc822_like::from_reader(reader)
}

/// Read buildinfo from a string
pub fn from_str(data: &str) -> Result<Buildinfo, rfc822_like::de::Error> {
    rfc822_like::from_str(data)
}

#[cfg(test)]
mod test {
    use crate::{architectures::Architecture, buildinfo::Buildinfo, version::PackageVersion};

    #[test]
    fn deserialize() {
        let data = r#"Format: 1.0
Source: picnic
Binary: libpicnic-dev libpicnic3 libpicnic3-dbgsym
Architecture: i386 source
Version: 3.0.11-1
Checksums-Md5:
 4b7826495233d2d3147ccaabead13a36 951 picnic_3.0.11-1.dsc
 1610e5affd53cf17b64b5727a32a6db9 10180 libpicnic-dev_3.0.11-1_i386.deb
 8229544cbadbe421e713fc247429b744 727956 libpicnic3-dbgsym_3.0.11-1_i386.deb
 924c58e34c3ff074850201984493a44d 619732 libpicnic3_3.0.11-1_i386.deb
Checksums-Sha1:
 e6cd8381339635aea7f4850105dbbdb5ac33e248 951 picnic_3.0.11-1.dsc
 d916aa5940e7c88fbe0fa420234c0a78db569c43 10180 libpicnic-dev_3.0.11-1_i386.deb
 383843942719b1a5fa8cdf2d768a4b1566c80f4d 727956 libpicnic3-dbgsym_3.0.11-1_i386.deb
 5fabd52dfee1258d2e9ac43f5d8c2f7ba61ca8cb 619732 libpicnic3_3.0.11-1_i386.deb
Checksums-Sha256:
 8b2a1969501be49fe11e8e8005bf9a3aac0e073d4c7fd97dcb8bfb6f8c9a222a 951 picnic_3.0.11-1.dsc
 96ab1c37ca12b0fb28169b79cf4a850ede58ab269abcc36eb6e36a6e66906b47 10180 libpicnic-dev_3.0.11-1_i386.deb
 d114fe20288c31fd2ac7644e1059fcc145788abff11325e84b0ad982ca486ed6 727956 libpicnic3-dbgsym_3.0.11-1_i386.deb
 012f9a5a27dabfc72c4d7010406e9635a747b9c4c42bc9554aaecc4c9edd0fee 619732 libpicnic3_3.0.11-1_i386.deb
Build-Origin: Debian
Build-Architecture: i386
Build-Date: Tue, 25 Jan 2022 21:54:55 +0000
Build-Path: /build/picnic-SQCH61/picnic-3.0.11
Installed-Build-Depends:
 autoconf (= 2.71-2),
 automake (= 1:1.16.5-1.1),
 autopoint (= 0.21-4),
 autotools-dev (= 20180224.1+nmu1),
 base-files (= 12.2),
 base-passwd (= 3.5.52),
 bash (= 5.1-6),
 binutils (= 2.37.90.20220123-1),
 binutils-common (= 2.37.90.20220123-1),
 binutils-i686-linux-gnu (= 2.37.90.20220123-1),
 bsdextrautils (= 2.37.3-1),
 bsdutils (= 1:2.37.3-1),
 build-essential (= 12.9),
 bzip2 (= 1.0.8-5),
 cmake (= 3.22.1-1+b1),
 cmake-data (= 3.22.1-1),
 coreutils (= 8.32-4.1),
 cpp (= 4:11.2.0-2),
 cpp-11 (= 11.2.0-14),
 dash (= 0.5.11+git20210903+057cd650a4ed-3),
 debconf (= 1.5.79),
 debhelper (= 13.6),
 debianutils (= 5.7-0.1),
 dh-autoreconf (= 20),
 dh-elpa-helper (= 2.0.10),
 dh-strip-nondeterminism (= 1.13.0-1),
 diffutils (= 1:3.7-5),
 dpkg (= 1.21.1),
 dpkg-dev (= 1.21.1),
 dwz (= 0.14-1),
 emacsen-common (= 3.0.4),
 file (= 1:5.41-2),
 findutils (= 4.8.0-1),
 g++ (= 4:11.2.0-2),
 g++-11 (= 11.2.0-14),
 gcc (= 4:11.2.0-2),
 gcc-11 (= 11.2.0-14),
 gcc-11-base (= 11.2.0-14),
 gettext (= 0.21-4),
 gettext-base (= 0.21-4),
 grep (= 3.7-1),
 groff-base (= 1.22.4-8),
 gzip (= 1.10-4),
 hostname (= 3.23),
 init-system-helpers (= 1.61),
 intltool-debian (= 0.35.0+20060710.5),
 libacl1 (= 2.3.1-1),
 libarchive-zip-perl (= 1.68-1),
 libarchive13 (= 3.5.2-1),
 libasan6 (= 11.2.0-14),
 libatomic1 (= 11.2.0-14),
 libattr1 (= 1:2.5.1-1),
 libaudit-common (= 1:3.0.6-1),
 libaudit1 (= 1:3.0.6-1+b1),
 libbinutils (= 2.37.90.20220123-1),
 libblkid1 (= 2.37.3-1),
 libboost-test-dev (= 1.74.0.3),
 libboost-test1.74-dev (= 1.74.0-14),
 libboost-test1.74.0 (= 1.74.0-14),
 libboost1.74-dev (= 1.74.0-14),
 libbrotli1 (= 1.0.9-2+b3),
 libbz2-1.0 (= 1.0.8-5),
 libc-bin (= 2.33-4),
 libc-dev-bin (= 2.33-4),
 libc6 (= 2.33-4),
 libc6-dev (= 2.33-4),
 libcap-ng0 (= 0.7.9-2.2+b1),
 libcap2 (= 1:2.44-1),
 libcc1-0 (= 11.2.0-14),
 libcom-err2 (= 1.46.5-2),
 libcrypt-dev (= 1:4.4.27-1.1),
 libcrypt1 (= 1:4.4.27-1.1),
 libctf-nobfd0 (= 2.37.90.20220123-1),
 libctf0 (= 2.37.90.20220123-1),
 libcurl4 (= 7.81.0-1),
 libdb5.3 (= 5.3.28+dfsg1-0.8),
 libdebconfclient0 (= 0.261),
 libdebhelper-perl (= 13.6),
 libdpkg-perl (= 1.21.1),
 libelf1 (= 0.186-1),
 libexpat1 (= 2.4.3-2),
 libffi8 (= 3.4.2-4),
 libfile-stripnondeterminism-perl (= 1.13.0-1),
 libgcc-11-dev (= 11.2.0-14),
 libgcc-s1 (= 11.2.0-14),
 libgcrypt20 (= 1.9.4-5),
 libgdbm-compat4 (= 1.22-1),
 libgdbm6 (= 1.22-1),
 libglib2.0-0 (= 2.70.2-1),
 libgmp10 (= 2:6.2.1+dfsg-3),
 libgnutls30 (= 3.7.3-4),
 libgomp1 (= 11.2.0-14),
 libgpg-error0 (= 1.43-3),
 libgssapi-krb5-2 (= 1.18.3-7),
 libhogweed6 (= 3.7.3-1),
 libicu67 (= 67.1-7),
 libidn2-0 (= 2.3.2-2),
 libisl23 (= 0.24-2),
 libitm1 (= 11.2.0-14),
 libjsoncpp25 (= 1.9.5-2),
 libk5crypto3 (= 1.18.3-7),
 libkeyutils1 (= 1.6.1-2),
 libkrb5-3 (= 1.18.3-7),
 libkrb5support0 (= 1.18.3-7),
 libldap-2.4-2 (= 2.4.59+dfsg-1),
 liblz4-1 (= 1.9.3-2),
 liblzma5 (= 5.2.5-2),
 libm4ri-0.0.20200125 (= 20200125-1+b1),
 libm4ri-dev (= 20200125-1+b1),
 libmagic-mgc (= 1:5.41-2),
 libmagic1 (= 1:5.41-2),
 libmount1 (= 2.37.3-1),
 libmpc3 (= 1.2.1-1),
 libmpfr6 (= 4.1.0-3),
 libncurses6 (= 6.3-2),
 libncursesw6 (= 6.3-2),
 libnettle8 (= 3.7.3-1),
 libnghttp2-14 (= 1.43.0-1),
 libnsl-dev (= 1.3.0-2),
 libnsl2 (= 1.3.0-2),
 libp11-kit0 (= 0.24.0-6),
 libpam-modules (= 1.4.0-11),
 libpam-modules-bin (= 1.4.0-11),
 libpam-runtime (= 1.4.0-11),
 libpam0g (= 1.4.0-11),
 libpcre2-8-0 (= 10.39-3),
 libpcre3 (= 2:8.39-13),
 libperl5.32 (= 5.32.1-6),
 libpipeline1 (= 1.5.5-1),
 libpng16-16 (= 1.6.37-3),
 libprocps8 (= 2:3.3.17-6),
 libpsl5 (= 0.21.0-1.2),
 libquadmath0 (= 11.2.0-14),
 librhash0 (= 1.4.2-1),
 librtmp1 (= 2.4+20151223.gitfa8646d.1-2+b2),
 libsasl2-2 (= 2.1.27+dfsg2-3),
 libsasl2-modules-db (= 2.1.27+dfsg2-3),
 libseccomp2 (= 2.5.3-2),
 libselinux1 (= 3.3-1+b1),
 libsigsegv2 (= 2.13-1),
 libsmartcols1 (= 2.37.3-1),
 libssh2-1 (= 1.10.0-2),
 libssl1.1 (= 1.1.1m-1),
 libstdc++-11-dev (= 11.2.0-14),
 libstdc++6 (= 11.2.0-14),
 libsub-override-perl (= 0.09-2),
 libsystemd0 (= 250.3-1),
 libtasn1-6 (= 4.18.0-4),
 libtinfo6 (= 6.3-2),
 libtirpc-common (= 1.3.2-2),
 libtirpc-dev (= 1.3.2-2),
 libtirpc3 (= 1.3.2-2),
 libtool (= 2.4.6-15),
 libubsan1 (= 11.2.0-14),
 libuchardet0 (= 0.0.7-1),
 libudev1 (= 250.3-1),
 libunistring2 (= 0.9.10-6),
 libuuid1 (= 2.37.3-1),
 libuv1 (= 1.43.0-1),
 libxml2 (= 2.9.12+dfsg-5+b1),
 libzstd1 (= 1.4.8+dfsg-3),
 linux-libc-dev (= 5.15.15-1),
 login (= 1:4.8.1-2),
 lsb-base (= 11.1.0),
 m4 (= 1.4.18-5),
 make (= 4.3-4.1),
 man-db (= 2.9.4-4),
 mawk (= 1.3.4.20200120-3),
 ncurses-base (= 6.3-2),
 ncurses-bin (= 6.3-2),
 patch (= 2.7.6-7),
 perl (= 5.32.1-6),
 perl-base (= 5.32.1-6),
 perl-modules-5.32 (= 5.32.1-6),
 pkg-config (= 0.29.2-1),
 po-debconf (= 1.0.21+nmu1),
 procps (= 2:3.3.17-6),
 rpcsvc-proto (= 1.4.2-4),
 sed (= 4.8-1),
 sensible-utils (= 0.0.17),
 sysvinit-utils (= 3.01-1),
 tar (= 1.34+dfsg-1),
 util-linux (= 2.37.3-1),
 xz-utils (= 5.2.5-2),
 zlib1g (= 1:1.2.11.dfsg-2)
Environment:
 DEB_BUILD_OPTIONS="parallel=5"
 SOURCE_DATE_EPOCH="1643116722""#;
        let buildinfo: Buildinfo = super::from_str(data).unwrap();
        assert_eq!(buildinfo.source, "picnic");
        assert_eq!(
            buildinfo.version,
            PackageVersion::try_from("3.0.11-1").unwrap()
        );
        assert_eq!(
            buildinfo.architecture,
            vec![Architecture::I386, Architecture::Source]
        );
    }
}
