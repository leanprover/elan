use crate::download::DownloadCfg;
use crate::errors::*;
use crate::manifestation::Manifestation;
use crate::prefix::InstallPrefix;
use elan_utils::{
    self,
    utils::{self},
};
use regex::Regex;
use serde_derive::Serialize;

use std::fmt;

// Fully-resolved toolchain descriptors. These always have full target
// triples attached to them and are used for canonical identification,
// such as naming their installation directory.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ToolchainDesc {
    // A linked toolchain
    Local {
        name: String,
    },
    Remote {
        // The GitHub source repository to use (if "nightly" is specified, we append "-nightly" to this).
        origin: String,
        // The release name, usually a Git tag
        release: String,
        // The channel name the release was resolved from, if any
        from_channel: Option<String>,
    },
}

impl ToolchainDesc {
    pub fn from_resolved_str(name: &str) -> Result<Self> {
        let pattern = r"^(?:([a-zA-Z0-9-]+[/][a-zA-Z0-9-]+)[:])?([a-zA-Z0-9-.]+)$";

        let re = Regex::new(&pattern).unwrap();
        if let Some(c) = re.captures(name) {
            match c.get(1) {
                Some(origin) => {
                    let origin = origin.as_str().to_owned();
                    let release = c.get(2).unwrap().as_str().to_owned();
                    Ok(ToolchainDesc::Remote {
                        origin,
                        release,
                        from_channel: None,
                    })
                }
                None => {
                    let name = c.get(2).unwrap().as_str().to_owned();
                    Ok(ToolchainDesc::Local { name })
                }
            }
        } else {
            Err(ErrorKind::InvalidToolchainName(name.to_string()).into())
        }
    }

    pub fn from_toolchain_dir(dir_name: &str) -> Result<Self> {
        // de-sanitize toolchain file names (best effort...)
        let name = dir_name.replace("---", ":").replace("--", "/");
        Self::from_resolved_str(&name)
    }
}

impl fmt::Display for ToolchainDesc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToolchainDesc::Local { name } => write!(f, "{}", name),
            ToolchainDesc::Remote {
                origin, release, ..
            } => write!(f, "{}:{}", origin, release),
        }
    }
}

pub fn install_from_dist<'a>(
    download: DownloadCfg<'a>,
    toolchain: &ToolchainDesc,
    prefix: &InstallPrefix,
) -> Result<()> {
    let toolchain_str = toolchain.to_string();
    let manifestation = Manifestation::open(prefix.clone())?;

    let ToolchainDesc::Remote {
        origin, release, ..
    } = toolchain
    else {
        return Ok(());
    };
    let res =
        match manifestation.install(&origin, &release, &download.temp_cfg, download.notify_handler) {
            Ok(()) => Ok(()),
            e
            @ Err(Error(ErrorKind::Utils(elan_utils::ErrorKind::DownloadNotExists { .. }), _)) => e
                .chain_err(|| {
                    format!(
                        "could not download nonexistent lean version `{}`",
                        toolchain_str
                    )
                }),
            Err(e) => Err(e),
        };

    // Don't leave behind an empty / broken installation directory
    if res.is_err() {
        // FIXME Ignoring cascading errors
        let _ = utils::remove_dir("toolchain", prefix.path(), &|n| {
            (download.notify_handler)(n.into())
        });
    }

    res
}

pub fn host_triple() -> &'static str {
    include_str!(concat!(env!("OUT_DIR"), "/target.txt"))
}
