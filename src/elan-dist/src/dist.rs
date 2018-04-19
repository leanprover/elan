
use temp;
use errors::*;
use elan_utils::{self, utils};
use prefix::InstallPrefix;
use manifest::Component;
use manifestation::{Manifestation};
use download::{DownloadCfg};
use notifications::Notification;

use std::path::Path;
use std::fmt;

use regex::Regex;

// Fully-resolved toolchain descriptors. These always have full target
// triples attached to them and are used for canonical identification,
// such as naming their installation directory.
#[derive(Debug, Clone)]
pub struct ToolchainDesc {
    // Either "nightly", "stable", or an explicit version number
    pub channel: String,
    pub date: Option<String>,
}

impl ToolchainDesc {
    pub fn from_str(name: &str) -> Result<Self> {
        let channels =
            ["nightly", "stable", r"\d{1}\.\d{1}\.\d{1}", r"\d{1}\.\d{2}\.\d{1}"];

        let pattern = format!(
            r"^({})(?:-(\d{{4}}-\d{{2}}-\d{{2}}))?$",
            channels.join("|"),
            );

        let re = Regex::new(&pattern).unwrap();
        re.captures(name)
            .map(|c| {
                fn fn_map(s: &str) -> Option<String> {
                    if s == "" {
                        None
                    } else {
                        Some(s.to_owned())
                    }
                }

                ToolchainDesc {
                    channel: c.get(1).unwrap().as_str().to_owned(),
                    date: c.get(2).map(|s| s.as_str()).and_then(fn_map),
                }
            })
            .ok_or(ErrorKind::InvalidToolchainName(name.to_string()).into())
    }

    /// Either "$channel" or "channel-$date"
    pub fn manifest_name(&self) -> String {
        match self.date {
            None => self.channel.clone(),
            Some(ref date) => format!("{}-{}", self.channel, date),
        }
    }

    pub fn package_dir(&self, dist_root: &str) -> String {
        match self.date {
            None => format!("{}", dist_root),
            Some(ref date) => format!("{}/{}", dist_root, date),
        }
    }

    pub fn full_spec(&self) -> String {
        if self.date.is_some() {
            format!("{}", self)
        } else {
            format!("{} (tracking)", self)
        }
    }

    pub fn is_tracking(&self) -> bool {
        let channels = ["nightly", "stable"];
        channels.iter().any(|x| *x == self.channel) && self.date.is_none()
    }
}

#[derive(Debug)]
pub struct Manifest<'a>(temp::File<'a>, String);

impl fmt::Display for ToolchainDesc {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "{}", &self.channel));

        if let Some(ref date) = self.date {
            try!(write!(f, "-{}", date));
        }

        Ok(())
    }
}


// Installs or updates a toolchain from a dist server. If an initial
// install then it will be installed with the default components. If
// an upgrade then all the existing components will be upgraded.
//
// Returns the manifest's hash if anything changed.
pub fn update_from_dist<'a>(download: DownloadCfg<'a>,
                            update_hash: Option<&Path>,
                            toolchain: &ToolchainDesc,
                            prefix: &InstallPrefix,
                            add: &[Component],
                            remove: &[Component],
                            force_update: bool)
                            -> Result<Option<String>> {

    let fresh_install = !prefix.path().exists();

    let res = update_from_dist_(download,
                                update_hash,
                                toolchain,
                                prefix,
                                add,
                                remove,
                                force_update);

    // Don't leave behind an empty / broken installation directory
    if res.is_err() && fresh_install {
        // FIXME Ignoring cascading errors
        let _ = utils::remove_dir("toolchain", prefix.path(),
                                  &|n| (download.notify_handler)(n.into()));
    }

    res
}

pub fn update_from_dist_<'a>(download: DownloadCfg<'a>,
                             update_hash: Option<&Path>,
                             toolchain: &ToolchainDesc,
                             prefix: &InstallPrefix,
                             _add: &[Component],
                             _remove: &[Component],
                             _force_update: bool)
                             -> Result<Option<String>> {

    let toolchain_str = toolchain.to_string();
    let manifestation = try!(Manifestation::open(prefix.clone()));

    let url = match toolchain_url(download, toolchain) {
        Ok(url) => url,
        Err(Error(ErrorKind::Utils(elan_utils::ErrorKind::DownloadNotExists { .. }), _)) => {
            return Err(format!("no release found for '{}'", toolchain.manifest_name()).into());
        }
        Err(e @ Error(ErrorKind::ChecksumFailed { .. }, _)) => {
            return Err(e);
        }
        Err(e) => {
            return Err(e).chain_err(|| {
                format!("failed to resolve latest version of '{}'",
                        toolchain.manifest_name())
            });
        }
    };

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

    match manifestation.update(&url,
                               &download.temp_cfg,
                               download.notify_handler.clone()) {
        Ok(()) => Ok(()),
        e @ Err(Error(ErrorKind::Utils(elan_utils::ErrorKind::DownloadNotExists { .. }), _)) => {
            e.chain_err(|| {
                format!("could not download nonexistent lean version `{}`",
                        toolchain_str)
            })
        }
        Err(e) => Err(e),
    }.map(|()| Some(url))
}

fn toolchain_url<'a>(download: DownloadCfg<'a>, toolchain: &ToolchainDesc) -> Result<String> {
    Ok(match (toolchain.date.as_ref(), toolchain.channel.as_str()) {
        (None, version) if version == "stable" || version == "nightly" => {
            (download.notify_handler)(Notification::DownloadingManifest(version));
            let repo = if version == "stable" { "leanprover/lean" } else { "leanprover/lean-nightly" };
            let release = utils::fetch_latest_release_tag(repo)?;
            (download.notify_handler)(Notification::DownloadedManifest(version, Some(&release)));
            format!("https://github.com/{}/releases/tag/{}", repo, release)
        }
        (Some(date), "nightly") =>
            format!("https://github.com/leanprover/lean-nightly/releases/tag/nightly-{}", date),
        (None, version) =>
            format!("https://github.com/leanprover/lean/releases/tag/v{}", version),
        _ => panic!("wat"),
    })
}

pub fn host_triple() -> &'static str {
    include_str!(concat!(env!("OUT_DIR"), "/target.txt"))
}