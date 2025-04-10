//! Installation and upgrade of both distribution-managed and local
//! toolchains

use crate::errors::Result;
use elan_dist::dist;
use elan_dist::download::DownloadCfg;
use elan_dist::prefix::InstallPrefix;
use elan_dist::Notification;
use elan_utils::utils::{self, fetch_latest_release_tag};
use std::path::Path;

#[cfg(feature = "no-self-update")]
pub const NEVER_SELF_UPDATE: bool = true;
#[cfg(not(feature = "no-self-update"))]
pub const NEVER_SELF_UPDATE: bool = false;

/// Downloads and returns new elan version string if not already up to date
pub fn check_self_update() -> Result<Option<String>> {
    // We should expect people that used their system package manger to install elan to also
    // regularly update those packages because otherwise we may repeatedly nag them about a new
    // version that is not even available to them yet
    if NEVER_SELF_UPDATE {
        return Ok(None);
    }

    // Get current version
    let current_version = env!("CARGO_PKG_VERSION");

    let tag = fetch_latest_release_tag("leanprover/elan", false)?;
    let available_version = &tag[1..];

    Ok(if available_version == current_version {
        None
    } else {
        Some(available_version.to_owned())
    })
}

#[derive(Copy, Clone)]
pub enum InstallMethod<'a> {
    Copy(&'a Path),
    Link(&'a Path),
    Dist(&'a dist::ToolchainDesc, DownloadCfg<'a>),
}

impl InstallMethod<'_> {
    pub fn run(self, path: &Path, notify_handler: &dyn Fn(Notification<'_>)) -> Result<()> {
        if path.exists() {
            // Don't uninstall first for Dist method
            match self {
                InstallMethod::Dist(..) => {}
                _ => {
                    uninstall(path, notify_handler)?;
                }
            }
        }

        match self {
            InstallMethod::Copy(src) => {
                utils::copy_dir(src, path, &|n| notify_handler(n.into()))?;
                Ok(())
            }
            InstallMethod::Link(src) => {
                utils::symlink_dir(src, path, &|n| notify_handler(n.into()))?;
                Ok(())
            }
            InstallMethod::Dist(toolchain, dl_cfg) => {
                if let Some(version) = check_self_update()? {
                    notify_handler(Notification::NewVersionAvailable(version));
                }

                let prefix = &InstallPrefix::from(path.to_owned());
                dist::install_from_dist(dl_cfg, toolchain, prefix)?;

                Ok(())
            }
        }
    }
}

pub fn uninstall(path: &Path, notify_handler: &dyn Fn(Notification<'_>)) -> Result<()> {
    Ok(utils::remove_dir("install", path, &|n| {
        notify_handler(n.into())
    })?)
}
