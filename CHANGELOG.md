# [Unreleased]

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
