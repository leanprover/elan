use crate::errors::*;
use crate::notifications::*;
use crate::temp;
use elan_utils::utils;

use std::ops;
use std::path::{Path, PathBuf};

const _UPDATE_HASH_LEN: usize = 20;

#[derive(Copy, Clone)]
pub struct DownloadCfg<'a> {
    pub temp_cfg: &'a temp::Cfg,
    pub notify_handler: &'a dyn Fn(Notification<'_>),
}

pub struct File {
    path: PathBuf,
}

impl ops::Deref for File {
    type Target = Path;

    fn deref(&self) -> &Path {
        ops::Deref::deref(&self.path)
    }
}

impl<'a> DownloadCfg<'a> {
    pub fn download_and_check(&self, url_str: &str) -> Result<temp::File<'a>> {
        let url = utils::parse_url(url_str)?;
        let file = self.temp_cfg.new_file()?;

        utils::download_file(&url, &file, &|n| (self.notify_handler)(n.into()))?;

        Ok(file)
    }
}
