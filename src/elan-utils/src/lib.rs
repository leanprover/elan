#![recursion_limit = "1024"] // for error_chain!

extern crate rand;
extern crate scopeguard;
#[macro_use]
extern crate error_chain;
extern crate curl;
extern crate dirs;
extern crate download;
extern crate regex;
extern crate semver;
extern crate sha2;
extern crate toml;
extern crate url;

#[cfg(windows)]
extern crate winapi;
#[cfg(windows)]
extern crate winreg;

#[cfg(unix)]
extern crate libc;

pub mod errors;
pub mod notifications;
pub mod raw;
pub mod toml_utils;
pub mod tty;
pub mod utils;

pub use errors::*;
pub use notifications::Notification;
pub mod notify;
