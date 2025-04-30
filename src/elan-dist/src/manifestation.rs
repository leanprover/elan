//! Manifest a particular Lean version by installing it from a distribution server.

use std::{thread::sleep, time::Duration};

use crate::component::{TarGzPackage, TarZstdPackage, ZipPackage};
use crate::download::DownloadCfg;
use crate::errors::*;
use crate::notifications::*;
use crate::prefix::InstallPrefix;
use crate::temp;
use elan_utils::utils::fetch_url;
use elan_utils::{raw::read_file, utils};
use fslock::LockFile;

pub const DEFAULT_ORIGIN: &str = "leanprover/lean4";
const DEFAULT_ORIGIN_JSON_URL: &str = "https://release.lean-lang.org";

pub fn get_json_uri_for_releases(origin: &str) -> Option<&str> {
    if origin == DEFAULT_ORIGIN || origin == DEFAULT_ORIGIN.to_owned() + "-nightly" {
        Some(DEFAULT_ORIGIN_JSON_URL)
    } else {
        None
    }
}

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
        release: &String,
        temp_cfg: &temp::Cfg,
        notify_handler: &dyn Fn(Notification<'_>),
    ) -> Result<()> {
        let prefix = self.prefix.path();
        utils::ensure_dir_exists("toolchains", prefix.parent().unwrap(), &|n| {
            (notify_handler)(n.into())
        })?;

        let lockfile_path = prefix.with_extension("lock");
        let mut lockfile = LockFile::open(&lockfile_path)?;
        if !lockfile.try_lock_with_pid()? {
            notify_handler(Notification::WaitingForFileLock(
                &lockfile_path,
                read_file(&lockfile_path)?.trim(),
            ));
            while !lockfile.try_lock_with_pid()? {
                sleep(Duration::from_secs(1));
            }
        }
        let res = self.do_install(origin, release, temp_cfg, notify_handler);
        let _ = std::fs::remove_file(&lockfile_path);
        res
    }

    fn do_install(
        &self,
        origin: &String,
        release: &String,
        temp_cfg: &temp::Cfg,
        notify_handler: &dyn Fn(Notification<'_>),
    ) -> Result<()> {
        let prefix = self.prefix.path();
        let dlcfg = DownloadCfg {
            temp_cfg: temp_cfg,
            notify_handler: notify_handler,
        };

        if utils::is_directory(prefix) {
            return Ok(());
        }

        // find correct download on HTML page (AAAAH)
        use regex::Regex;
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
        // For historical reasons, the informal target for Linux x64 is a substring of Linux
        // aarch64; make sure we don't confuse them
        let name_substring = informal_target.clone() + ".";
        let url = if let Some(url) = get_json_uri_for_releases(origin) {
            let json = fetch_url(url)?;
            let releases = json::parse(&json)
                .chain_err(|| format!("failed to parse release data: {}", url))?;
            let release = releases.entries().flat_map(|(_, channel)| channel.members())
                .find(|release_obj| release_obj["name"].as_str() == Some(release))
                .ok_or_else(|| format!("no such release: '{}'", release))?;
            let asset = release["assets"].members()
                .find(|asset| asset["name"].as_str().iter().any(|name| name.contains(&name_substring)))
                .ok_or_else(|| format!("binary package was not provided for '{}'", informal_target))?;
            asset["browser_download_url"].as_str().unwrap().to_owned()
        } else {
            let url = format!(
                "https://github.com/{}/releases/expanded_assets/{}",
                origin, release
            );
            let re = Regex::new(format!(r#"/{}/releases/download/[^"]+"#, origin).as_str()).unwrap();
            let html = fetch_url(&url)?;
            let url = re
                .find_iter(&html)
                .map(|m| m.as_str().to_string())
                .find(|m| m.contains(&name_substring));
            if url.is_none() {
                return Err(
                    format!("binary package was not provided for '{}'", informal_target).into(),
                );
            }
            format!("https://github.com{}", url.unwrap())
        };
        notify_handler(Notification::DownloadingComponent(&url));

        let installer_file = dlcfg.download_and_check(&url)?;

        notify_handler(Notification::InstallingComponent(&prefix.to_string_lossy()));

        // unpack into temporary place, then move atomically to guard against aborts during unpacking
        let unpack_dir = prefix.with_extension("tmp");

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
            return Err(format!("unsupported archive format: {}", url).into());
        }

        utils::rename_dir("temp toolchain directory", &unpack_dir, prefix)?;

        Ok(())
    }
}
