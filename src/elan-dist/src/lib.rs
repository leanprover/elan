#![recursion_limit = "1024"]

extern crate elan_utils;
extern crate flate2;
extern crate itertools;
extern crate regex;
extern crate tar;
extern crate toml;
extern crate url;
extern crate walkdir;
#[macro_use]
extern crate error_chain;
extern crate json;
extern crate sha2;
extern crate time;
extern crate zip;

#[cfg(not(windows))]
extern crate libc;
#[cfg(windows)]
extern crate winapi;
#[cfg(windows)]
extern crate winreg;

pub use errors::*;
pub use notifications::Notification;

pub mod temp;

mod component;
pub mod config;
pub mod dist;
pub mod download;
pub mod errors;
pub mod manifest;
mod manifestation;
pub mod notifications;
pub mod prefix;
