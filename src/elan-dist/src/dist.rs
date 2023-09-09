use download::DownloadCfg;
use elan_utils::utils::fetch_url;
use elan_utils::{self, utils};
use errors::*;
use manifest::Component;
use manifestation::Manifestation;
use prefix::InstallPrefix;
use temp;

use std::fmt;
use std::path::Path;

use regex::Regex;

const DEFAULT_ORIGIN: &str = "leanprover/lean4";

// Fully-resolved toolchain descriptors. These always have full target
// triples attached to them and are used for canonical identification,
// such as naming their installation directory.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolchainDesc {
    // The GitHub source repository to use (if "nightly" is specified, we append "-nightly" to this).
    // Defaults to `DEFAULT_ORIGIN`.
    pub origin: String,
    // The release name, usually a Git tag
    pub release: String,
}

impl ToolchainDesc {
    pub fn from_str(name: &str) -> Result<Self> {
        let pattern = r"^(?:([a-zA-Z0-9-]+[/][a-zA-Z0-9-]+)[:])?([a-zA-Z0-9-.]+)$";

        let re = Regex::new(&pattern).unwrap();
        if let Some(c) = re.captures(name) {
            let mut origin = c.get(1).map(|s| s.as_str()).unwrap_or(DEFAULT_ORIGIN).to_owned();
            let mut release = c.get(2).unwrap().as_str().to_owned();
            if release.starts_with("nightly") && !origin.ends_with("-nightly") {
                origin = format!("{}-nightly", origin);
            }
            if release == "lean-toolchain" {
                let toolchain_url = format!("https://raw.githubusercontent.com/{}/HEAD/lean-toolchain", origin);
                return ToolchainDesc::from_str(fetch_url(&toolchain_url)?.trim())
            }
            if release == "stable" || release == "nightly" {
                release = utils::fetch_latest_release_tag(&origin)?;
            }
            if release.starts_with(char::is_numeric) {
                release = format!("v{}", release)
            }
            Ok(ToolchainDesc { origin, release })
        } else {
            Err(ErrorKind::InvalidToolchainName(name.to_string()).into())
        }
    }

    pub fn manifest_name(&self) -> String {
        self.release.clone()
    }

    pub fn is_tracking(&self) -> bool {
        return false
    }

    fn url(&self) -> String {
        format!("https://github.com/{}/releases/expanded_assets/{}", self.origin, self.release)
    }
}

#[derive(Debug)]
pub struct Manifest<'a>(temp::File<'a>, String);

impl fmt::Display for ToolchainDesc {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.origin, self.release)
    }
}

// Installs or updates a toolchain from a dist server. If an initial
// install then it will be installed with the default components. If
// an upgrade then all the existing components will be upgraded.
//
// Returns the manifest's hash if anything changed.
pub fn update_from_dist<'a>(
    download: DownloadCfg<'a>,
    update_hash: Option<&Path>,
    toolchain: &ToolchainDesc,
    prefix: &InstallPrefix,
    add: &[Component],
    remove: &[Component],
    force_update: bool,
) -> Result<Option<String>> {
    let fresh_install = !prefix.path().exists();

    let res = update_from_dist_(
        download,
        update_hash,
        toolchain,
        prefix,
        add,
        remove,
        force_update,
    );

    // Don't leave behind an empty / broken installation directory
    if res.is_err() && fresh_install {
        // FIXME Ignoring cascading errors
        let _ = utils::remove_dir("toolchain", prefix.path(), &|n| {
            (download.notify_handler)(n.into())
        });
    }

    res
}

pub fn update_from_dist_<'a>(
    download: DownloadCfg<'a>,
    update_hash: Option<&Path>,
    toolchain: &ToolchainDesc,
    prefix: &InstallPrefix,
    _add: &[Component],
    _remove: &[Component],
    _force_update: bool,
) -> Result<Option<String>> {
    let toolchain_str = toolchain.to_string();
    let manifestation = Manifestation::open(prefix.clone())?;

    let url = toolchain.url();

    if let Some(hash_file) = update_hash {
        if utils::is_file(hash_file) {
            if let Ok(contents) = utils::read_file("update hash", hash_file) {
                if contents == url {
                    // Skip download, url matches
                    return Ok(None);
                }
            } /*else {
                  (self.notify_handler)(Notification::CantReadUpdateHash(hash_file));
              }*/
        } /*else {
              (self.notify_handler)(Notification::NoUpdateHash(hash_file));
          }*/
    }

    match manifestation.update(
        &toolchain.origin,
        &url,
        &download.temp_cfg,
        download.notify_handler.clone(),
    ) {
        Ok(()) => Ok(()),
        e @ Err(Error(ErrorKind::Utils(elan_utils::ErrorKind::DownloadNotExists { .. }), _)) => e
            .chain_err(|| {
                format!(
                    "could not download nonexistent lean version `{}`",
                    toolchain_str
                )
            }),
        Err(e) => Err(e),
    }
    .map(|()| Some(url))
}

pub fn host_triple() -> &'static str {
    include_str!(concat!(env!("OUT_DIR"), "/target.txt"))
}
