//! The main elan commandline application
//!
//! The elan binary is a chimera, changing its behavior based on the
//! name of the binary. This is used most prominently to enable
//! elan's tool 'proxies' - that is, elan itself and the elan
//! proxies are the same binary; when the binary is called 'elan' or
//! 'elan.exe' elan behaves like the elan commandline
//! application; when it is called 'lean' it behaves as a proxy to
//! 'lean'.
//!
//! This scheme is further used to distingush the elan installer,
//! called 'elan-init' which is again just the elan binary under a
//! different name.

#![recursion_limit = "1024"]
#![deny(rust_2018_idioms)]

#[macro_use]
mod log;
mod common;
mod download_tracker;
mod elan_mode;
mod errors;
mod help;
mod job;
mod json_dump;
mod proxy_mode;
mod self_update;
mod setup_mode;
mod term2;

use elan::env_var::LEAN_RECURSION_COUNT_MAX;
use errors::*;
use std::env;
use std::path::PathBuf;

fn main() {
    if let Err(ref e) = run_elan() {
        common::report_error(e);
        std::process::exit(1);
    }
}

fn run_elan() -> Result<()> {
    // Guard against infinite proxy recursion. This mostly happens due to
    // bugs in elan.
    do_recursion_guard()?;

    // The name of arg0 determines how the program is going to behave
    let arg0 = env::args().next().map(PathBuf::from);
    let name = arg0
        .as_ref()
        .and_then(|a| a.file_stem())
        .and_then(|a| a.to_str());

    match name {
        Some("elan") => elan_mode::main(),
        Some(n) if n.starts_with("elan-setup") || n.starts_with("elan-init") => {
            // NB: The above check is only for the prefix of the file
            // name. Browsers rename duplicates to
            // e.g. elan-setup(2), and this allows all variations
            // to work.
            setup_mode::main()
        }
        Some(n) if n.starts_with("elan-gc-") => {
            // This is the final uninstallation stage on windows where
            // elan deletes its own exe
            self_update::complete_windows_uninstall()
        }
        Some(_) => proxy_mode::main(),
        None => {
            // Weird case. No arg0, or it's unparsable.
            Err(ErrorKind::NoExeName.into())
        }
    }
}

fn do_recursion_guard() -> Result<()> {
    let recursion_count = env::var("LEAN_RECURSION_COUNT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    if recursion_count > LEAN_RECURSION_COUNT_MAX {
        return Err(ErrorKind::InfiniteRecursion.into());
    }

    Ok(())
}
