# Tools for Debian Release Team work

This crate includes tools to help with typical work of Debian's release team. Currently, it processes `excuses.yaml` to check for packages that require a binNMU for migration to testing, prepares a list of binNMUs for transitions, and so on.

## Usage:

`drt-tools` supports common options:
* `-n`: Generate a list of `wb` commands, but do not schedule them.
* `-f,--force`: Force processing even if some conditions are not met.

The following commands are provided:

* `grep-excuses`: Mostly the same as the tool of the same name from `devscripts`.
* `process-excuses`: Download and process `excuses.yaml` to generate a list binNMUs for packages that require them for migration. Also processes packages that require an unblock to migrate to testing (e.g., for uploads to tpu or during freeze).
* `nmu-transition`: Take a list packages copies from [ben's output](https://release.debian.org/transitions) and schedules binNMUs. This command supports multiple options:
   * `-m message`: the binNMU message
   * `--dw dependency`: additionally generate a `dw` command with the given dependency
   * `--extra-depends dependency`: schedule the binNMUs with an extra dependency
   * `--bp priority`: specify a build priority
   * `--suite suite`: specify a suite
   * `-a architecture`: use a different architecture than `ANY`
* `nmu-eso`: Produce and schedule a list of rebuilds for packages having Built-Using on source packages with `Extra-Source-Only: yes` set. This command supports the following options:
   * `--bp priority`: specifiy a build priority (default: -50)
   * `--suite suite`: specify a suite

## License

Copyright 2021-2025 Sebastian Ramacher

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <https://www.gnu.org/licenses/>.
