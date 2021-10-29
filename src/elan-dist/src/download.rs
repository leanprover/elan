use elan_utils::utils;
use errors::*;
use notifications::*;
use sha2::{Digest, Sha256};
use temp;
use url::Url;

use std::fs;
use std::ops;
use std::path::{Path, PathBuf};

const _UPDATE_HASH_LEN: usize = 20;

#[derive(Copy, Clone)]
pub struct DownloadCfg<'a> {
    pub temp_cfg: &'a temp::Cfg,
    pub download_dir: &'a PathBuf,
    pub notify_handler: &'a dyn Fn(Notification),
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
    /// Downloads a file, validating its hash, and resuming interrupted downloads
    /// Partial downloads are stored in `self.download_dir`, keyed by hash. If the
    /// target file already exists, then the hash is checked and it is returned
    /// immediately without re-downloading.
    pub fn download(&self, url: &Url, hash: &str) -> Result<File> {
        utils::ensure_dir_exists("Download Directory", &self.download_dir, &|n| {
            (self.notify_handler)(n.into())
        })?;
        let target_file = self.download_dir.join(Path::new(hash));

        if target_file.exists() {
            let cached_result = file_hash(&target_file)?;
            if hash == cached_result {
                (self.notify_handler)(Notification::FileAlreadyDownloaded);
                (self.notify_handler)(Notification::ChecksumValid(&url.to_string()));
                return Ok(File { path: target_file });
            } else {
                (self.notify_handler)(Notification::CachedFileChecksumFailed);
                fs::remove_file(&target_file).chain_err(|| "cleaning up previous download")?;
            }
        }

        let partial_file_path = target_file.with_file_name(
            target_file
                .file_name()
                .map(|s| s.to_str().unwrap_or("_"))
                .unwrap_or("_")
                .to_owned()
                + ".partial",
        );

        let mut hasher = Sha256::new();

        utils::download_file_with_resume(
            &url,
            &partial_file_path,
            Some(&mut hasher),
            true,
            &|n| (self.notify_handler)(n.into()),
        )?;

        let actual_hash = format!("{:x}", hasher.finalize());

        if hash != actual_hash {
            // Incorrect hash
            return Err(ErrorKind::ChecksumFailed {
                url: url.to_string(),
                expected: hash.to_string(),
                calculated: actual_hash,
            }
            .into());
        } else {
            (self.notify_handler)(Notification::ChecksumValid(&url.to_string()));
            fs::rename(&partial_file_path, &target_file)?;
            return Ok(File { path: target_file });
        }
    }

    pub fn clean(&self, hashes: &Vec<String>) -> Result<()> {
        for hash in hashes.iter() {
            let used_file = self.download_dir.join(hash);
            if self.download_dir.join(&used_file).exists() {
                fs::remove_file(used_file).chain_err(|| "cleaning up cached downloads")?;
            }
        }
        Ok(())
    }

    pub fn download_and_check(&self, url_str: &str) -> Result<temp::File<'a>> {
        let url = utils::parse_url(url_str)?;
        let file = self.temp_cfg.new_file()?;

        utils::download_file(&url, &file, None, &|n| (self.notify_handler)(n.into()))?;

        Ok(file)
    }
}

fn file_hash(path: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    use std::io::Read;
    let mut downloaded = fs::File::open(&path).chain_err(|| "opening already downloaded file")?;
    let mut buf = vec![0; 32768];
    loop {
        if let Ok(n) = downloaded.read(&mut buf) {
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        } else {
            break;
        }
    }

    Ok(format!("{:x}", hasher.finalize()))
}
