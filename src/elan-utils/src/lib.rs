#![recursion_limit = "1024"] // for error_chain!
#![deny(rust_2018_idioms)]

pub mod errors;
pub mod notifications;
pub mod raw;
pub mod toml_utils;
pub mod tty;
pub mod utils;

pub use errors::*;
pub use notifications::Notification;
pub mod notify;
