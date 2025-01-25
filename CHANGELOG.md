# Changelog

## 0.2.27 (2025-01-25)

* Bump MSRV to 1.84 for MSRV-aware resolver.
* `nmu-list`: Also supported whitespace separated list of packages.
* Build manpages and shell completion.

## 0.2.26 (2025-01-01)

* `nmu-eso`: Produce architecture binNMU commands if possible.

## 0.2.25 (2024-11-30)

* `nmu-list`: New command

## 0.2.24 (2024-08-15)

* `process-excuses`: Merge `process-unblocks`.
* `nmu-transitions`: Allow `--arch` to take `wb` architecture.

## 0.2.23 (2024-06-09)

* `nmu-t64`: Removed, no longer needed.

## 0.2.22 (2024-04-27)

* `nmu-t64`: Handle more packages where t64 changes have been reverted.
* Fix scheduling of binNMUs via `wuiet.debian.org`.

## 0.2.21 (2024-04-07)

* `nmu-eso`:
  - Add support for `X-Cargo-Built-Using`.
  - Make command more robust.
* Schedule builds by directly connecting to `wuiet.debian.org`.
* `usrmerged`: Removed, no longer needed.
* `nmu-t64`: New command to schedule binNMUs for the time_t 64-bit transition.

## 0.2.20 (2024-02-17)

* Handle more releases.
* `nmu-eso`: Add support for `Static-Built-Using`.

## 0.2.19 (2024-02-02)

* Parse `Release` files to determine list of architectures.
* Remove use of `regex` crate.
* Handle HTTP errors proberly.

## 0.2.18 (2023-11-21)

* Some internal refactoring
* Bump `itertools` to 0.12.

## 0.2.17 (2023-09-14)

* Implement new sub-command:
  * `nmu-version-skew`: binNMU MA: same packages which out-of-sync versions
* Bump MSRV to 1.70 due to dependencies

## 0.2.16 (2023-06-11)

* Fix handling of `verbose` and `quiet` flags.

## 0.2.15 (2023-06-10)

* Upgrade `assorted-debian-utils` to 0.6.
* Bump MSRV to 1.65

## 0.2.14 (2023-04-15)

* Bump MSRV to 1.64
* `usrmerged`: Find more moved files.

## 0.2.13 (2023-03-16)

* `nmu-eso`: Take `Package` and `Source` files from the archive to build list of packages needing a rebuild.
* `nmu-eso`: Only skip `debian-installer` and `-signed` packages.

## 0.2.12 (2023-02-25)

* `nmu-eso`: Skip `-signed` packages.
* `nmu-eso`: Mention cause for rebuilds in binNMU message.
* Refactor command implementaton.

## 0.2.11 (2023-01-17)

* Implement new sub-command:
  * `process-unblocks`: Produce a list of packages that require unblocks.

## 0.2.10 (2022-10-30)

* Download compressed migration excuses.

## 0.2.9 (2022-10-02)

* `binNMU-buildinfo`: Various improvements.
* Upgrade to `clap` 4.

## 0.2.8 (2022-09-18)

* `binNMU-buildinfo`: Make command usable again.

## 0.2.7 (2022-09-09)

* Reduce feature flags of dependencies.
* Make mirror for package files configurable.
* `nmu-eso`: Reduce options and provide sensible defaults.

## 0.2.6 (2022-07-09)

* Implement new sub-command:
  * `usrmerged`: check file moves between /usr and /
* Implement parallel downloads

## 0.2.5 (2022-05-22)

* `nmu-eso`: Skip packages that FTBFS.

## 0.2.4 (2022-04-25)

* Implement new sub-command:
  * grep-excuses: Similar to `grep-excuses(1)`.
  * `nmu-eso`: Rebuild packages with outdated Built-Using.
* `prepare-binNMUs`: Skip packages with FTBFS bugs.

## 0.2.3 (2022-04-06)

* Update to assorted-debian-utils 0.4.0.
* `process-excuses`: Correctly skip packages that would require a binNMU for arch: all.

## 0.2.2 (2022-01-30)

* Implement new sub-command:
  * binnmu-buildinfo: Based on a list of buildinfo files, schedule binNMUs.
* Migrate from `structopt` to `clap` 3.

## 0.2.1 (2021-12-22)

* Bump to rfc822-like 0.2.1 and deserialize `Multi-Arch` as enum.

## 0.2 (2021-12-12)

* Implement sub-commands:
  * process-excuses: Covers the old functionality
  * prepare-binNMUs: Generate a list of binNMUs for a transition

## 0.1 (2021-11-17)

* Initial release.
