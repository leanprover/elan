# Unreleased

- Revamp toolchain resolution and storage
  * Toolchain references are now always resolved to a fixed `<owner>/<repo>:<version>` format before
    downloading, resolving the issue of having the same toolchain installed as e.g. `stable`,
    `v4.9.0`, and `leanprover/lean4:v4.9.0` and ensuring that e.g. `stable` has the same meaning for
    everyone at a given point in time.
  * Accordingly, `elan update` has been removed as updates of non-fixed toolchains are now implicit.
  * This also fixes `lake +stable new ...` putting an unadvisable `leanprover/lean4:stable`
    reference in `lean-toolchain`.
  * The configured default toolchain name is stored unresolved (and not immediately installed) but
    is resolved before every use. This also means that passing `--default-toolchain none` to the
    elan install script is not necessary anymore to speed up installation.
  * In case of a network error resolving a toolchain, elan falls back to the previous resolution if
    any and prints a warning.
  * The `update-hashes/` directory is not used anymore, deleting `toolchains/` or direct
    subdirectories does not break elan anymore.
- More useful download and installation info messages
- Experimental `elan toolchain gc` command. See `elan toolchain gc --help` for documentation.
  `lean-toolchain` files will only be known to the GC after being used at least once with this
  version of elan.

# 3.1.1 - 2024-02-22

- Fix update check

# 3.1.0 - 2024-02-19

- Fix partial toolchains potentially being left in place when unpacking is aborted (#121)
- Check for elan updates during toolchain downloads (#122)

# 3.0.0 - 2023-09-08

- Default to stable toolchain again in time for first Lean 4 stable release (#104)

# 2.0.1 - 2023-07-24

- Fix download progress display on Windows (#101)

# 2.0.0 - 2023-07-03

- Support toolchain reference `<origin>:lean-toolchain` that refers to the toolchain referred to by the contents of the given GitHub file (#99)

- Default to Lean 4 (#98)

# 1.4.6 - 2023-06-10

- Avoid dependency on the VC++ Redistributable on Windows (#97)

# 1.4.5 - 2023-04-26

- Fix self update on Apple Silicon (only?) (#78)

# 1.4.4 - 2023-04-24

- Update dependencies

# 1.4.3 - 2023-04-24

- Avoid setting `(DY)LD_LIBRARY_PATH` (#90)

# 1.4.2 - 2022-09-13

- Fix downloading Lean releases again

# 1.4.1 - 2022-04-15

## Added

- Actual support for ARM64 macOS (M1)

# 1.4.0 - 2022-03-28

## Added

- Support for ARM64 macOS (M1)

# 1.3.1 - 2021-11-01

## Changed

- Update dependencies

# 1.3.0 - 2021-11-01

## Added

- Support for zstd-compressed tarballs

# 1.2.0 - 2021-10-19

## Added

- Support for ARM64 Linux

# 1.1.2 - 2021-10-15

## Changed

- Remove another "press any key to exit"

# 1.1.1 - 2021-10-15

## Changed

- Remove "press any key to exit" step from Windows installation not needed for VS Code or PowerShell method

# 1.1.0 - 2021-10-08

## Added

- Add `lake` Lean 4 executable

# 1.0.8 - 2021-09-10

## Changed

- Fix `elan self update` on not-Linux, again

# 1.0.7 - 2021-08-16

## Changed

- Default to respective toolchain inside of `~/.elan` (#36)

# 1.0.6 - 2021-05-25

## Changed

- Fix `elan self update` on not-Linux and build from cmdline

# 1.0.5 - 2021-05-25

## Changed

- Run extension-less tools such as `leanc` using `sh` on Windows (and hope for the best...)

# 1.0.4 - 2021-05-24

## Changed

- Update suggestion when no default toolchain is configured (#31)
- Fix `elan show` when no default toolchain is configured (#33)

# 1.0.3 - 2021-04-30

## Changed

- Fix `elan self update` download URL on Linux

# 1.0.2 - 2021-04-28

## Changed

- Fix installation from non-default repos

# 1.0.1 - 2021-04-28

## Changed

- Fix updating channels from non-default repos (e.g. `leanprover/lean4:nightly`)
  This change affects the store location of such toolchains, so you will have to re-install them first.
  ```sh
  $ elan toolchain uninstall leanprover-lean4-nightly
  $ elan toolchain install leanprover/lean4:nightly
  ```

# 1.0.0 - 2021-04-17

- Move to `leanprover/elan`

# 0.11.0 - 2021-03-09

## Changed

- Make `elan` a static executable on Linux
- Improve `leanpkg.toml` error handling (#26)
- Make downloaded files read-only (on Linux/macOS) (#27)

# 0.10.3 - 2021-01-15

## Changed

- Hopefully fix Lean 4 leanpkg on Windows

# 0.10.2 - 2020-05-11

## Changed

- Hopefully actually restore `elan toolchain link` functionality

# 0.10.1 - 2020-05-11

## Changed

- Hopefully restore `elan toolchain link` functionality

# 0.10.0 - 2020-05-08

## Changed

- Accept (almost) arbitrary release tag names in addition to version numbers

# 0.9.0 - 2020-05-07

## Added

- Add `leanc`, `leanmake` Lean 4 executables

# 0.8.0 - 2020-03-06

## Changed

- stable/nightly now refer to leanprover-community, Lean's community fork. This includes the toolchain installed by default (stable).

# 0.7.5 - 2019-03-21

## Changed

- Fix release lookup once more with feeling

# 0.7.4 - 2019-03-20

## Changed

- Fix self-update always triggering

# 0.7.3 - 2019-03-20

## Changed

- Fix lookup of latest Github release of both Lean and elan

# 0.7.2 - 2019-01-15

## Changed

- Fix name check in `elan toolchain link` (#17)

# 0.7.0 - 2018-09-16

## Added

- elan will now warn if there are other Lean installations in the PATH before installing

## Changed

- Fix mtimes not being restored from installation archives
- Fix invoking leanpkg on Windows

# 0.6.0 - 2018-08-01

## Added

- Version specifiers can now point to custom forks of Lean, such as `khoek/klean:3.4.1` (#8)

# 0.5.0 - 2018-04-20

## Changed

- An explicit version passed to a proxy command like in `leanpkg +nightly build` will now be installed automatically when necessary
- Full toolchain names and their directories do not mention the operating system (the "target triple", to be exact) any more. You may want to delete your old toolchains from `~/.elan/toolchains` to save space.

# [0.4.0 - 2018-04-17]

## Changed

- `leanpkg.toml` and `lean-toolchain` files can now reference custom toolchains (those added by `elan toolchain link`)

# [0.3.0] - 2018-04-11

## Added

- `leanchecker` proxy

# [0.2.0] - 2018-04-11

## Added

- `curl | sh` installation and instructions

## Changed

- Fix `elan toolchain link` (#1)
- Fix self-update
- De-rustify docs

# [0.1.0] - 2018-04-10

Minimum viable product release

## Added

- Building on Rustup's code, implement installing and managing Lean toolchains
- Have leanpkg.toml files override the Lean version
