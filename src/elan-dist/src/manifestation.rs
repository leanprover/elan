//! Manifest a particular Lean version by installing it from a distribution server.

use component::{TarGzPackage, ZipPackage};
use temp;
use errors::*;
use notifications::*;
use download::DownloadCfg;
use prefix::InstallPrefix;

#[derive(Debug)]
pub struct Manifestation {
    prefix: InstallPrefix
}

impl Manifestation {
    pub fn open(prefix: InstallPrefix) -> Result<Self> {
        Ok(Manifestation { prefix })
    }

    /// Installation using the legacy v1 manifest format
    pub fn update(&self,
                  origin: &String,
                  url: &String,
                  temp_cfg: &temp::Cfg,
                  notify_handler: &Fn(Notification)) -> Result<()> {
        notify_handler(Notification::DownloadingComponent("lean"));

        use std::path::PathBuf;
        let dld_dir = PathBuf::from("bogus");
        let dlcfg = DownloadCfg {
            download_dir: &dld_dir,
            temp_cfg: temp_cfg,
            notify_handler: notify_handler
        };

        // find correct download on HTML page (AAAAH)
        use std::fs;
        use regex::Regex;
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
        let re = Regex::new(format!(r#"/{}/releases/download/[^"]+"#, origin).as_str()).unwrap();
        let download_page_file = dlcfg.download_and_check(&url, "")?;
        let mut html = String::new();
        fs::File::open(&download_page_file as &::std::path::Path)?.read_to_string(&mut html)?;
        let url = re.find_iter(&html).map(|m| m.as_str().to_string()).find(|m|
            m.contains(informal_target));
        if url.is_none() {
            return Err(format!("binary package was not provided for '{}'",
                               informal_target).into());
        }
        let url = format!("https://github.com/{}", url.unwrap());

        let ext = if cfg!(target_os = "linux") { ".tar.gz" } else { ".zip" };
        let installer_file = try!(dlcfg.download_and_check(&url, ext));

        let prefix = self.prefix.path();
        let prefix = &prefix;

        notify_handler(Notification::InstallingComponent("lean"));

        // Remove old files
        for entry in fs::read_dir(prefix)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                fs::remove_dir_all(entry.path())?
            } else {
                fs::remove_file(entry.path())?
            }
        }

        // Extract new files
        if cfg!(target_os = "linux") {
            TarGzPackage::unpack_file(&installer_file, prefix)?
        } else {
            ZipPackage::unpack_file(&installer_file, prefix)?
        };

        Ok(())
    }
}
