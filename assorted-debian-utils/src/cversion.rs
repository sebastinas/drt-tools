// Copyright 2022 Sebastian Ramacher
// SPDX-License-Identifier: LGPL-3.0-or-later

use std::{cmp::Ordering, ffi::CString};

use crate::version::PackageVersion;

/// A helper struct storing C-compatible strings for `dpkg_version`
pub(crate) struct CVersion {
    epoch: u32,
    upstream_version: CString,
    debian_revision: CString,
}

impl From<&PackageVersion> for CVersion {
    fn from(version: &PackageVersion) -> Self {
        // never null
        let upstream_version = CString::new(version.upstream_version.as_str()).unwrap();
        // never null
        let debian_revision = CString::new(
            version
                .debian_revision
                .as_ref()
                .map_or_else(|| "", |v| v.as_str()),
        )
        .unwrap();

        Self {
            epoch: version.epoch_or_0(),
            upstream_version,
            debian_revision,
        }
    }
}

impl CVersion {
    fn as_dpkg_version(&self) -> libdpkg_sys::dpkg_version {
        libdpkg_sys::dpkg_version {
            epoch: self.epoch,
            version: self.upstream_version.as_ptr(),
            revision: self.debian_revision.as_ptr(),
        }
    }
}

impl Ord for CVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        let ret = unsafe {
            libdpkg_sys::dpkg_version_compare(&self.as_dpkg_version(), &other.as_dpkg_version())
        };
        ret.cmp(&0)
    }
}

impl PartialOrd for CVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for CVersion {}

impl PartialEq for CVersion {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
