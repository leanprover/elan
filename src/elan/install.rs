//! Installation and upgrade of both distribution-managed and local
//! toolchains

use elan_dist::dist;
use elan_dist::download::DownloadCfg;
use elan_dist::prefix::InstallPrefix;
use elan_dist::Notification;
use elan_utils::utils::{self, fetch_latest_release_tag};
use errors::Result;
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
        return Ok(None)
    }

    // Get current version
    let current_version = env!("CARGO_PKG_VERSION");

    let tag = fetch_latest_release_tag("leanprover/elan")?;
    let available_version = &tag[1..];

    dbg!((available_version, current_version));
    Ok(if available_version != current_version { None } else { Some(available_version.to_owned()) })
}

#[derive(Copy, Clone)]
pub enum InstallMethod<'a> {
    Copy(&'a Path),
    Link(&'a Path),
    // bool is whether to force an update
    Dist(
        &'a dist::ToolchainDesc,
        Option<&'a Path>,
        DownloadCfg<'a>,
        bool,
    ),
}

impl<'a> InstallMethod<'a> {
    pub fn run(self, path: &Path, notify_handler: &dyn Fn(Notification)) -> Result<bool> {
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
                Ok(true)
            }
            InstallMethod::Link(src) => {
                utils::symlink_dir(src, &path, &|n| notify_handler(n.into()))?;
                Ok(true)
            }
            InstallMethod::Dist(toolchain, update_hash, dl_cfg, force_update) => {
                if let Some(version) = check_self_update()? {
                    notify_handler(Notification::NewVersionAvailable(version));
                }

                let prefix = &InstallPrefix::from(path.to_owned());
                let maybe_new_hash = dist::update_from_dist(
                    dl_cfg,
                    update_hash,
                    toolchain,
                    prefix,
                    &[],
                    &[],
                    force_update,
                )?;

                if let Some(hash) = maybe_new_hash {
                    if let Some(hash_file) = update_hash {
                        utils::write_file("update hash", hash_file, &hash)?;
                    }

                    Ok(true)
                } else {
                    Ok(false)
                }
            }
        }
    }
}

pub fn uninstall(path: &Path, notify_handler: &dyn Fn(Notification)) -> Result<()> {
    Ok(utils::remove_dir("install", path, &|n| {
        notify_handler(n.into())
    })?)
}
