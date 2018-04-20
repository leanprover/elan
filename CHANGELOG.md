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
