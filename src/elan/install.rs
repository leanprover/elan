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
    Dist(
        &'a dist::ToolchainDesc,
        DownloadCfg<'a>,
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
            InstallMethod::Dist(toolchain, dl_cfg) => {
                let prefix = &InstallPrefix::from(path.to_owned());
                dist::install_from_dist(
                    dl_cfg,
                    toolchain,
                    prefix,
                )?;

                Ok(true)
            }
        }
    }
}

pub fn uninstall(path: &Path, notify_handler: &dyn Fn(Notification)) -> Result<()> {
    Ok(utils::remove_dir("install", path, &|n| {
        notify_handler(n.into())
    })?)
}
