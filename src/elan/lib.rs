#![recursion_limit = "1024"]
#![deny(rust_2018_idioms)]

pub use crate::errors::*;
pub use config::*;
pub use elan_utils::{notify, toml_utils, utils};
pub use notifications::*;
pub use toolchain::*;

pub mod command;
mod config;
pub mod env_var;
mod errors;
pub mod gc;
pub mod install;
mod notifications;
pub mod settings;
mod toolchain;
