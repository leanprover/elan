# elan: Lean version manager

<table>
  <tr>
    <th>License</th><th>Windows</th><th>Linux / OS X</th>
  </tr>
  <tr>
    <td><a href="LICENSE"><img src="https://img.shields.io/badge/license-APACHE_2-green.svg?dummy" title="License"/></a></td>
    <td><a href="https://ci.appveyor.com/project/Kha/elan"><img src="https://ci.appveyor.com/api/projects/status/56t26ufeo25q99sw/branch/master"/></a></td>
    <td><a href="https://travis-ci.org/Kha/elan"><img src="https://travis-ci.org/Kha/elan.png?branch=master"/></a></td>
  </tr>
</table>

*elan* is a small tool for managing your installations of the [Lean theorem prover](https://leanprover.github.io). It places `lean` and `leanpkg` binaries in your `PATH` that automatically select and, if necessary, download the Lean version described in the `lean_version` field of your project's `leanpkg.toml`.
You can also install, select, run, and uninstall Lean versions manually using the commands of the `elan` executable.

# Installation

Run the following command in a bash-like shell and follow the printed instructions:

```bash
curl https://sh.rustup.rs -sSf | sh
```

TODO: Provide a better installation method on Windows

# Implementation

*elan* is basically a fork of [rustup](https://github.com/rust-lang-nursery/rustup.rs). Apart from new features and adaptions to the Lean infrastructure, these are the basic changes to the original code:

* Replaced every mention of `rustup` with `elan`, `cargo` with `leanpkg`, and `rust(c)` with `lean`
* Removed Windows installer... for now?
* Merged `CARGO_HOME` and `RUSTUP_HOME`
* Removed options to configure host triple
