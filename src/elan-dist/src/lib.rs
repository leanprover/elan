#![recursion_limit = "1024"]
#![deny(rust_2018_idioms)]

pub use errors::*;
pub use notifications::Notification;

pub mod temp;

mod component;
pub mod config;
pub mod dist;
pub mod download;
pub mod errors;
pub mod manifest;
pub mod manifestation;
pub mod notifications;
pub mod prefix;
