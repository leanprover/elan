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
