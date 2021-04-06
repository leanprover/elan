# elan: Lean version manager

*elan* is a small tool for managing your installations of the [Lean theorem prover](https://leanprover.github.io). It places `lean` and `leanpkg` binaries in your `PATH` that automatically select and, if necessary, download the Lean version described in the `lean_version` field of your project's `leanpkg.toml`.
You can also install, select, run, and uninstall Lean versions manually using the commands of the `elan` executable.

```bash
~/my/package $ cat leanpkg.toml | grep lean_version
lean_version = "nightly-2018-04-10"
~/my/package $ leanpkg -v
info: downloading component 'lean'
 14.6 MiB /  14.6 MiB (100 %)   2.2 MiB/s ETA:   0 s
info: installing component 'lean'
Lean package manager, version nightly-2018-04-10
[...]
~/my/package $ elan show
installed toolchains
--------------------

stable
nightly-2018-04-06
nightly-2018-04-10
master

active toolchain
----------------

nightly-2018-04-10 (overridden by '/home/me/my/package/leanpkg.toml')
Lean (version 3.3.1, nightly-2018-04-10, commit d36b859c6579, Release)
```

# Installation

**Linux/OS X/Cygwin/MSYS2/git bash/...**: Run the following command in a terminal:

```bash
curl https://raw.githubusercontent.com/Kha/elan/master/elan-init.sh -sSf | sh
```

Alternatively, **any supported platform**: Grab the [latest release](https://github.com/Kha/elan/releases/latest) for your platform, unpack it, and run it in a terminal.

The installation will tell you where it will install elan to (`~/.elan` by default), and also ask you about editing your shell config to extend `PATH`. elan can be uninstalled via `elan self uninstall`, which should revert these changes.

# Prerequisites

On some systems `elan` will not work out of the box:

* Windows 10
   * You'll need a unix-like terminal.
      * Recommended: "git bash", available via [Git for Windows](https://gitforwindows.org/) works well, or
      * the terminal from [MSYS2](https://www.msys2.org/), and then run `pacman -S unzip git`
* macOS: Install [Homebrew](https://brew.sh/), then run `brew install gmp coreutils`.
  (`gmp` is required by `lean`, `coreutils` is required by `leanpkg`)

# Implementation

*elan* is basically a fork of [rustup](https://github.com/rust-lang-nursery/rustup.rs). Apart from new features and adaptions to the Lean infrastructure, these are the basic changes to the original code:

* Replaced every mention of `rustup` with `elan`, `cargo` with `leanpkg`, and `rust(c)` with `lean`
* Removed Windows installer... for now?
* Merged `CARGO_HOME` and `RUSTUP_HOME`
* Removed options to configure host triple
