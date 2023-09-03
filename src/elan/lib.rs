#![recursion_limit = "1024"]

extern crate elan_dist;
extern crate elan_utils;
#[macro_use]
extern crate error_chain;
extern crate itertools;
#[cfg(unix)]
extern crate libc;
extern crate regex;
extern crate serde_derive;
extern crate serde_json;
extern crate tempfile;
extern crate time;
extern crate toml;
extern crate url;

pub use config::*;
pub use elan_utils::{notify, toml_utils, utils};
pub use errors::*;
pub use notifications::*;
pub use toolchain::*;

pub mod command;
mod config;
pub mod env_var;
mod errors;
mod install;
mod notifications;
pub mod settings;
mod toolchain;
