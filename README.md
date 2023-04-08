# Tools for Debian Release Team work

This crate includes tools to help with typical work of Debian's release team. Currently, it processes `excuses.yaml` to check for packages that require a binNMU for migration to testing, prepares a list of binNMUs for transitions, and so on.

## Usage:

`drt-tools` supports common options:
* `-n`: Generate a list of `wb` commands, but do not schedule them.
* `-f,--force`: Force processing even if some conditions are not met.

The following commands are provided:

* `grep-excuses`: Mostly the same as the tool of the same name from `devscripts`.
* `process-excuses`: Download and process `excuses.yaml` to generate a list binNMUs for packages that require them for migration. Packages that have other issues preventing them from migrating, are not considered.
* `process-unblocks`: Download and process `excuses.yaml` to generate a list of unblocks for packages in tpu and some other cases.
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
* `usrmerged`: Check for packages moving files from / to /usr or vice versa.


## License

This crate is Copyright 2021-2022 Sebastian Ramacher and licensed under the GPL version 3.0 or later.
