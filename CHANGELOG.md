# Changelog

## 0.2.4 (2022-04-25)

* Implement new sub-command:
  * grep-excuses: similar to `grep-excuses(1)`.
  * `nmu-eso`: Rebuild packages with outdated Built-Using.
* `prepare-binNMUs`: skip packages with FTBFS bugs.

## 0.2.3 (2022-04-06)

* Update to assorted-debian-utils 0.4.0.
* `process-excuses`: correctly skip packages that would require a binNMU for arch: all.

## 0.2.2 (2022-01-30)

* Implement new sub-command:
  * binnmu-buildinfo: based on a list of buildinfo files, schedule binNMUs.
* Migrate from `structopt` to `clap` 3.

## 0.2.1 (2021-12-22)

* Bump to rfc822-like 0.2.1 and deserialize `Multi-Arch` as enum.

## 0.2 (2021-12-12)

* Implement sub-commands:
  * process-excuses: covers the old functionality
  * prepare-binNMUs: generate a list of binNMUs for a transition

## 0.1 (2021-11-17)

* Initial release.
