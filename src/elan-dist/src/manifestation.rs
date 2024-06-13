//! Manifest a particular Lean version by installing it from a distribution server.

use component::{TarGzPackage, TarZstdPackage, ZipPackage};
use download::DownloadCfg;
use elan_utils::utils;
use errors::*;
use notifications::*;
use prefix::InstallPrefix;
use temp;

#[derive(Debug)]
pub struct Manifestation {
    prefix: InstallPrefix,
}

impl Manifestation {
    pub fn open(prefix: InstallPrefix) -> Result<Self> {
        Ok(Manifestation { prefix })
    }

    pub fn install(
        &self,
        origin: &String,
        url: &String,
        temp_cfg: &temp::Cfg,
        notify_handler: &dyn Fn(Notification),
    ) -> Result<()> {
        let dlcfg = DownloadCfg {
            temp_cfg: temp_cfg,
            notify_handler: notify_handler,
        };

        // find correct download on HTML page (AAAAH)
        use regex::Regex;
        use std::fs;
        use std::io::Read;
        let informal_target = if cfg!(target_os = "windows") {
            "windows"
        } else if cfg!(target_os = "linux") {
            "linux"
        } else if cfg!(target_os = "macos") {
            "darwin"
        } else {
            unreachable!()
        };
        let informal_target = informal_target.to_owned();
        let informal_target = if cfg!(target_arch = "x86_64") {
            informal_target
        } else if cfg!(target_arch = "aarch64") {
            informal_target + "_aarch64"
        } else {
            unreachable!();
        };
        let url_substring = informal_target.clone() + ".";
        let re = Regex::new(format!(r#"/{}/releases/download/[^"]+"#, origin).as_str()).unwrap();
        let download_page_file = dlcfg.download_and_check(&url)?;
        let mut html = String::new();
        fs::File::open(&download_page_file as &::std::path::Path)?.read_to_string(&mut html)?;
        let url = re
            .find_iter(&html)
            .map(|m| m.as_str().to_string())
            .find(|m| m.contains(&url_substring));
        if url.is_none() {
            return Err(
                format!("binary package was not provided for '{}'", informal_target).into(),
            );
        }
        let url = format!("https://github.com/{}", url.unwrap());
        notify_handler(Notification::DownloadingComponent(&url));

        let installer_file = dlcfg.download_and_check(&url)?;

        let prefix = self.prefix.path();

        notify_handler(Notification::InstallingComponent(&prefix.to_string_lossy()));

        // unpack into temporary place, then move atomically to guard against aborts during unpacking
        let unpack_dir = prefix.with_extension("tmp");

        if utils::is_directory(prefix) {
            return Err(format!("'{}' is already installed", prefix.display()).into())
        }

        if utils::is_directory(&unpack_dir) {
            utils::remove_dir("temp toolchain directory", &unpack_dir, &|n| {
                (notify_handler)(n.into())
            })?;
        }

        utils::ensure_dir_exists("temp toolchain directory", &unpack_dir, &|n| {
            (notify_handler)(n.into())
        })?;

        // Extract new files
        if url.ends_with(".tar.gz") {
            TarGzPackage::unpack_file(&installer_file, &unpack_dir)?
        } else if url.ends_with(".tar.zst") {
            TarZstdPackage::unpack_file(&installer_file, &unpack_dir)?
        } else if url.ends_with(".zip") {
            ZipPackage::unpack_file(&installer_file, &unpack_dir)?
        } else {
            return Err(format!("unsupported archive format: {}", url).into())
        }

        utils::rename_dir("temp toolchain directory", &unpack_dir, prefix)?;

        Ok(())
    }
}
