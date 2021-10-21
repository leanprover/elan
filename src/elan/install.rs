//! Installation and upgrade of both distribution-managed and local
//! toolchains

use elan_dist::dist;
use elan_dist::download::DownloadCfg;
use elan_dist::prefix::InstallPrefix;
use elan_dist::Notification;
use elan_utils::utils;
use errors::Result;
use std::path::Path;

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
    pub fn run(self, path: &Path, notify_handler: &Fn(Notification)) -> Result<bool> {
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

pub fn uninstall(path: &Path, notify_handler: &Fn(Notification)) -> Result<()> {
    Ok(utils::remove_dir("install", path, &|n| {
        notify_handler(n.into())
    })?)
}
