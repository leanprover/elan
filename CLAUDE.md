# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**elan** is a toolchain version manager for the Lean theorem prover, forked from rustup. It manages multiple Lean installations and automatically selects the correct version based on `lean-toolchain` files in projects. The binary is a chimera that changes behavior based on its invocation name:
- `elan`/`elan-init`: Main CLI application and installer
- `lean`, `lake`, etc.: Proxies that delegate to the correct toolchain version

## Build Commands

### Basic Build
```bash
cargo build
# Output: target/debug/elan-init
```

### Release Build
```bash
cargo build --release
# Output: target/release/elan-init
```

### Cross-compilation (used in CI)
```bash
cargo install cross --locked
cross build --release --target <TARGET>
# Examples: x86_64-unknown-linux-musl, aarch64-apple-darwin, x86_64-pc-windows-msvc
```

### Testing
```bash
cargo test --release --target <TARGET>
```

### Windows Build Requirements
- 64-bit developer command prompt
- Windows version of perl.exe from https://strawberryperl.com/ (must be first in PATH, not Git's perl)

## Architecture

### Binary Dispatch (src/elan-cli/main.rs)
The main entry point determines behavior by examining arg0:
- `elan` → `elan_mode::main()` - CLI commands
- `elan-init` / `elan-setup*` → `setup_mode::main()` - Installation
- `elan-gc-*` → `self_update::complete_windows_uninstall()` - Windows cleanup
- Any other name → `proxy_mode::main()` - Tool proxying (lean, lake, etc.)

Recursion guard via `LEAN_RECURSION_COUNT` environment variable prevents infinite proxy loops.

### Workspace Structure
This is a Cargo workspace with four crates:
- `elan` (src/elan/lib.rs): Core library with config, toolchain resolution, and installation logic
- `elan-init` (src/elan-cli/): Main binary and CLI implementation
- `elan-dist` (src/elan-dist/): Distribution management, manifests, and component installation
- `elan-utils` (src/elan-utils/): Shared utilities
- `download` (src/download/): HTTP download functionality (curl-backend or reqwest-backend)

### Key Concepts

**Toolchain Resolution** (src/elan/toolchain.rs, src/elan/config.rs):
- Toolchain descriptors can be:
  - `Remote`: Downloaded from GitHub releases (e.g., `leanprover/lean4:v4.0.0`)
  - `Local`: Custom linked toolchains
- Resolution hierarchy checks (in order):
  1. `ELAN_TOOLCHAIN` environment variable
  2. Directory override (`elan override set`)
  3. `lean-toolchain` file in current/parent directories
  4. `leanpkg.toml` file (legacy)
  5. Inside a toolchain directory itself
  6. Default toolchain from settings

**Proxy Mode** (src/elan-cli/proxy_mode.rs):
When elan is called as `lean`, `lake`, etc., it:
1. Checks for `+toolchain` as first argument for explicit toolchain selection
2. Resolves which toolchain to use for current directory
3. Executes the real binary from that toolchain's directory

**Configuration** (src/elan/config.rs):
- ELAN_HOME: `~/.elan` (configurable)
- Settings file: `~/.elan/settings.toml` (default toolchain, overrides)
- Toolchains directory: `~/.elan/toolchains/`
- Temporary directory: `~/.elan/tmp/`

### CLI Commands (src/elan-cli/elan_mode.rs)
Main commands implemented:
- `show`: Display active and installed toolchains
- `install/uninstall`: Manage toolchains
- `default`: Set default toolchain
- `override set/unset/list`: Per-directory toolchain overrides
- `toolchain link`: Create custom toolchain from local directory
- `toolchain gc`: Garbage-collect unused toolchains
- `run`: Run command with specific toolchain
- `which`: Show path to binary
- `doc/man`: Open documentation
- `self update/uninstall`: Manage elan itself
- `completions`: Generate shell completions

### Download Backend
Two features control HTTP backend (default: curl-backend):
- `curl-backend`: Uses libcurl
- `reqwest-backend`: Pure Rust alternative

### Feature Flags
- `no-self-update`: Disable self-update functionality (for package managers)
- `msi-installed`: Changes self-update/uninstall behavior for Windows MSI installations

## Installation Flow
1. `elan-init.sh` or `elan-init.ps1` downloads and runs `elan-init` binary
2. Setup mode installs to `~/.elan`, adds to PATH via shell config
3. Creates symlinks: `elan`, `lean`, `lake` → `elan-init`
4. Downloads default toolchain on first use

## Commit Messages

Write the subject as `<type>: <summary>` (e.g. `fix: path-based toolchain as override`). GitHub appends ` (#<PR>)` automatically on merge — do not add it yourself. If the commit fixes an issue, reference it in the body (e.g. `Fixes #156.`), not in the subject. A body is optional; when present, separate it from the subject with a blank line and write each paragraph as one long line — do not hard-wrap inside a paragraph. Blank lines between paragraphs are fine.

## Changelog

User-facing fixes and features must add a bullet to `CHANGELOG.md` in the same commit. Add to (or create) the top `# Unreleased` section — at release time it becomes `# X.Y.Z - YYYY-MM-DD`. Pure internal refactors, test-only changes, and `chore:` commits don't need an entry.

## Testing Strategy
No unit tests (marked `test = false` in Cargo.toml). Integration testing via:
- CI runs build, test, and install test on all platforms
- Install test: runs `elan-init -y`, creates new lake project, builds it

## Development Notes

### Rustup Legacy
This codebase is a fork of rustup with terminology changes:
- rustup → elan
- cargo → lake
- rust(c) → lean
- CARGO_HOME merged with RUSTUP_HOME

### Toolchain Naming
Toolchains follow pattern: `[origin/][owner/repo:]release`
- Default origin: `leanprover/lean4` (becomes `leanprover/lean4-nightly` for nightly builds)
- Nightly releases: `nightly-YYYY-MM-DD`
- Stable releases: `vX.Y.Z` or `X.Y.Z` (auto-prefixed with `v`)
- Special channels: `stable`, `beta`, `nightly`, `lean-toolchain` (resolve to latest local)

### NixOS Support
Toolchains require patching on NixOS - handled by Nixpkgs version. See `fetch_nixos_patch.sh`.
