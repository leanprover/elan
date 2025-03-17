# elan: Lean version manager

*elan* is a small tool for managing your installations of the [Lean theorem prover](https://leanprover.github.io). It places `lean` and `lake` binaries in your `PATH` that automatically select and, if necessary, download the Lean version described in your project's `lean-toolchain` file.
You can also install, select, run, and uninstall Lean versions manually using the commands of the `elan` executable.

```shell
~/my/package $ cat lean-toolchain
nightly-2023-06-27

~/my/package $ lake --version
info: downloading component 'lean'
Total: 181.0 MiB Speed:  17.7 MiB/s
info: installing component 'lean'
Lake version 4.1.0-pre (Lean version 4.0.0-nightly-2023-06-27)

~/my/package $ elan show
installed toolchains
--------------------

nightly (default)
nightly-2022-06-27

active toolchain
----------------

nightly-2023-06-27 (overridden by '/home/me/my/package/lean-toolchain')
Lean (version 4.0.0-nightly-2023-06-27, commit bb8cc08de85f, Release)
```

# Installation

## Manual Installation

**Linux/macOS/Cygwin/MSYS2/git bash/...**: run the following command in a terminal:

```bash
curl https://elan.lean-lang.org/elan-init.sh -sSf | sh
```

**Windows**: run the following commands in a terminal (Command Prompt or PowerShell ≥ version 7.4.1):
```bash
curl -O --location https://elan.lean-lang.org/elan-init.ps1
powershell -ExecutionPolicy Bypass -f elan-init.ps1
del elan-init.ps1
```

Alternatively, on **any supported platform**: Grab the [latest release](https://github.com/leanprover/elan/releases/latest) for your platform, unpack it, and run the contained installation program.

The installation will tell you where it will install elan to (`~/.elan` by default), and also ask you about editing your shell config to extend `PATH`. elan can be uninstalled via `elan self uninstall`, which should revert these changes.

## NixOS

The toolchains downloaded by elan require some patching on NixOS, which is done automatically by the version available in Nixpkgs.
```bash
$ nix-env -iA nixpkgs.elan
```

# Prerequisites

On some systems, `lake` will not work out of the box even if installed through elan:

* You'll need [git](https://git-scm.com/download) to download dependencies through `lake`.

# Implementation

*elan* is basically a fork of [rustup](https://github.com/rust-lang-nursery/rustup.rs). Apart from new features and adaptions to the Lean infrastructure, these are the basic changes to the original code:

* Replaced every mention of `rustup` with `elan`, `cargo` with `lake`, and `rust(c)` with `lean`
* Merged `CARGO_HOME` and `RUSTUP_HOME`
* Removed options to configure host triple

# Build

If you want to build elan from source, you will need to install [Rust](https://www.rust-lang.org/tools/install) and
Cargo and run the following:

```
cargo build
```

The `elan-init` installer will show up in `target/debug`. This is also the main `elan` executable, so can test that it works by running the following:

```
ln -s ./target/debug/elan-init ./elan
./elan --help
```

## Build on Windows

The windows build requires a 64-bit developer command prompt and a Windows version of `perl.exe` which you can download
from [https://strawberryperl.com/](https://strawberryperl.com/). Make sure this downloaded perl.exe is the first thing
in your PATH so that the build does not try and use `C:\Program Files\Git\usr\bin\perl.exe`. The git provided version of
perl doesn't work for some reason.

Then you can run `cargo build` as shown above.
