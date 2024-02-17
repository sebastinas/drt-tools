# Changelog

## 0.7.0 (2024-02-17)

* Remove obsolete `version-compare` feature.
* Handle binNMU versions of native packages.
* Rewrite tests without spectral.

## 0.6.3 (2024-02-06)

* Fix serialization of codenames and suites.
* Parse `Release` files to obtain list of architectures.
* Deprecate the use of `RELEASE_ARCHITECTURES` and `ARCHIVE_ARCHITECTURES`.

## 0.6.2 (2023-09-14)

* Remove `mipsel` from unstable architectures.

## 0.6.1 (2023-08-08)

* Remove `mipsel` from release architectures.

## 0.6.0 (2023-06-10)

* Update for release of Debian bookworm.

## 0.5.7 (2023-02-25)

* Implement `AsRef<str>` for enums where possible.

## 0.5.6 (2023-01-17)

* Make overall verdict available.
* Add missing verdicts.

## 0.5.5 (2022-10-29)

* Implement package version comparison without `libdpkg-sys`.
* Fix handling of some package versions that are valid but were not accepted.

## 0.5.4 (2022-10-06)

* Add support for `arc` architecture
* Fix `m68k` and `riscv64` architectures
* Add support for `non-free-firmware` component

## 0.5.3 (2022-10-02)

* Add `version-compare` feature (replaces optional `libdpkg-sys` dependency).
* Re-export `rfc822_like`.

## 0.5.2 (2022-09-09)

* Derive `Eq` where possible.
* Bump `serde_yaml` to 0.9.

## 0.5.1 (2022-07-09)

* Implement errors with `thiserror`.
* excuses: Add tests.
* excuses: Add some helper functions.

## 0.5 (2022-05-22)

* Implement Clone and Copy consistently for enums.
* wb: Rename MinusArchitecture to ExcludeArchitecture.
* wb: Add builder for `wb fail`.
* excuses: Move Component to archive.

## 0.4.1 (2022-04-24)

* Add parser for auto-removals.
* Add enum for Multi-Arch fields.

## 0.4.0 (2022-04-06)

* Add initial support for codenames and suites.
* Provide proper comparison for package versions via `libdpkg-sys`.

## 0.3.1 (2022-02-01)

* Fail on binNMUs for `all`.

## 0.3 (2022-01-30)

* Add parser for `.buildinfo` files.
* Handle invalid/unsupported architectures in `wb`.

## 0.2 (2021-12-12)

* Extend `wb` support.

## 0.1 (2021-11-17)

* Initial release.
