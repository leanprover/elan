use download::DownloadCfg;
use elan_utils::utils::fetch_url;
use elan_utils::{self, utils};
use errors::*;
use manifestation::Manifestation;
use prefix::InstallPrefix;
use temp;

use std::fmt;

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

pub fn install_from_dist<'a>(
    download: DownloadCfg<'a>,
    toolchain: &ToolchainDesc,
    prefix: &InstallPrefix,
) -> Result<()> {
    let toolchain_str = toolchain.to_string();
    let manifestation = Manifestation::open(prefix.clone())?;

    let url = toolchain.url();

    let res = match manifestation.install(
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
    };

    // Don't leave behind an empty / broken installation directory
    if res.is_err() {
        // FIXME Ignoring cascading errors
        let _ = utils::remove_dir("toolchain", prefix.path(), &|n| {
            (download.notify_handler)(n.into())
        });
    }

    res
}

pub fn host_triple() -> &'static str {
    include_str!(concat!(env!("OUT_DIR"), "/target.txt"))
}
