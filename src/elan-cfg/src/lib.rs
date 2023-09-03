//! Crate for config Elan properties, mostly for URLs
//! Currently, those URLs follow the endpoint structure of GitHub releases heavily. It is not neat for downstream mirrors to mirror necessary meta info and binaries.

use once_cell::sync::Lazy;

macro_rules! env_var_or_default {
    ($var:expr, $default:expr) => {{
        Lazy::new(move || std::env::var($var).unwrap_or($default.into()))
    }};
}

pub static ELAN_UPDATE_ROOT: Lazy<String> = env_var_or_default!(
    "ELAN_UPDATE_ROOT",
    "https://github.com/leanprover/elan/releases/download"
);

pub static RELEASE_ROOT: Lazy<String> = env_var_or_default!("RELEASE_ROOT", "https://github.com");
