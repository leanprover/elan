use crate::config::Cfg;
use crate::env_var;
use crate::errors::*;
use crate::install::{self, InstallMethod};
use crate::notifications::*;
use elan_dist::dist::ToolchainDesc;
use elan_dist::download::DownloadCfg;
use elan_dist::manifest::Component;
use elan_dist::manifestation::get_json_uri_for_releases;
use elan_dist::manifestation::DEFAULT_ORIGIN;
use elan_utils::utils;
use elan_utils::utils::fetch_url;
use itertools::Itertools;

use regex::Regex;
use serde_derive::Serialize;
use std::env;
use std::env::consts::EXE_SUFFIX;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A fully resolved reference to a toolchain which may or may not exist
pub struct Toolchain<'a> {
    cfg: &'a Cfg,
    pub desc: ToolchainDesc,
    path: PathBuf,
    dist_handler: Box<dyn Fn(elan_dist::Notification<'_>) + 'a>,
}

/// Used by the `list_component` function
pub struct ComponentStatus {
    pub component: Component,
    pub required: bool,
    pub installed: bool,
    pub available: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UnresolvedToolchainDesc(pub ToolchainDesc);

pub fn lookup_unresolved_toolchain_desc(cfg: &Cfg, name: &str) -> Result<UnresolvedToolchainDesc> {
    let pattern = r"^(?:([a-zA-Z0-9-_]+[/][a-zA-Z0-9-_]+)[:])?([a-zA-Z0-9-.]+)$";

    let re = Regex::new(pattern).unwrap();
    if let Some(c) = re.captures(name) {
        let mut release = c.get(2).unwrap().as_str().to_owned();
        let local_tc = Toolchain::from(
            cfg,
            &ToolchainDesc::Local {
                name: release.clone(),
            },
        );
        if local_tc.exists() && local_tc.is_custom() {
            return Ok(UnresolvedToolchainDesc(ToolchainDesc::Local {
                name: release,
            }));
        }
        let mut origin = c
            .get(1)
            .map(|s| s.as_str())
            .unwrap_or(DEFAULT_ORIGIN)
            .to_owned();
        if release.starts_with("nightly") && !origin.ends_with("-nightly") {
            origin = format!("{}-nightly", origin);
        }
        let mut from_channel = None;
        if release == "lean-toolchain"
            || release == "stable"
            || release == "beta"
            || release == "nightly"
        {
            from_channel = Some(release.to_string());
        }
        if release.starts_with(char::is_numeric) {
            release = format!("v{}", release)
        }
        Ok(UnresolvedToolchainDesc(ToolchainDesc::Remote {
            origin,
            release,
            from_channel,
        }))
    } else {
        Err(ErrorKind::InvalidToolchainName(name.to_string()).into())
    }
}

fn find_latest_local_toolchain(cfg: &Cfg, channel: &str) -> Option<ToolchainDesc> {
    let toolchains = cfg.list_toolchains().ok()?;
    let toolchains = toolchains.into_iter().filter_map(|tc| match tc {
        ToolchainDesc::Remote { release: ref r, .. } => Some((tc.to_owned(), r.to_string())),
        _ => None,
    });
    let toolchains: Vec<_> = match channel {
        "nightly" => toolchains
            .filter(|t| t.1.starts_with("nightly-"))
            .sorted_by_key(|t| t.1.to_string())
            .map(|t| t.0)
            .collect(),
        _ => toolchains
            .filter_map(|t| {
                semver::Version::parse(t.1.trim_start_matches("v"))
                    .ok()
                    .filter(|v| (channel == "stable") == v.pre.is_empty())
                    .map(|v| (t.0, v))
            })
            .sorted_by_key(|t| t.1.clone())
            .map(|t| t.0)
            .collect(),
    };
    toolchains.into_iter().last()
}

pub fn resolve_toolchain_desc_ext(
    cfg: &Cfg,
    unresolved_tc: &UnresolvedToolchainDesc,
    no_net: bool,
    use_cache: bool,
) -> Result<ToolchainDesc> {
    if let ToolchainDesc::Remote {
        ref origin,
        ref release,
        from_channel: Some(ref channel),
    } = unresolved_tc.0
    {
        if release == "lean-toolchain" {
            let toolchain_url = format!(
                "https://raw.githubusercontent.com/{}/HEAD/lean-toolchain",
                origin
            );
            resolve_toolchain_desc_ext(
                cfg,
                &lookup_unresolved_toolchain_desc(cfg, fetch_url(&toolchain_url)?.trim())?,
                no_net,
                use_cache,
            )
        } else if release == "stable" || release == "beta" || release == "nightly" {
            let fetch = if let Some(uri) = get_json_uri_for_releases(origin) {
                utils::fetch_latest_release_json(uri, release, no_net)
            } else {
                if release == "beta" {
                    return Err(Error::from(
                        format!("channel 'beta' is not supported for custom origin '{}'", origin)
                    ));
                }
                utils::fetch_latest_release_tag(origin, no_net)
            };
            match fetch {
                Ok(release) => Ok(ToolchainDesc::Remote {
                    origin: origin.clone(),
                    release,
                    from_channel: Some(channel.clone()),
                }),
                Err(e) => {
                    if let (true, Some(tc)) = (use_cache, find_latest_local_toolchain(cfg, release))
                    {
                        if !no_net {
                            (cfg.notify_handler)(Notification::UsingExistingRelease(&tc));
                        }
                        Ok(tc)
                    } else {
                        Err(e)?
                    }
                }
            }
        } else {
            Ok(unresolved_tc.0.clone())
        }
    } else {
        Ok(unresolved_tc.0.clone())
    }
}

pub fn resolve_toolchain_desc(
    cfg: &Cfg,
    unresolved_tc: &UnresolvedToolchainDesc,
) -> Result<ToolchainDesc> {
    resolve_toolchain_desc_ext(cfg, unresolved_tc, false, true)
}

pub fn lookup_toolchain_desc(cfg: &Cfg, name: &str) -> Result<ToolchainDesc> {
    resolve_toolchain_desc(cfg, &lookup_unresolved_toolchain_desc(cfg, name)?)
}

pub fn read_unresolved_toolchain_desc_from_file(
    cfg: &Cfg,
    toolchain_file: &Path,
) -> Result<UnresolvedToolchainDesc> {
    let s = utils::read_file("toolchain file", toolchain_file)?;
    if let Some(s) = s.lines().next() {
        let toolchain_name = s.trim();
        lookup_unresolved_toolchain_desc(cfg, toolchain_name)
    } else {
        Err(Error::from(format!(
            "empty toolchain file '{}'",
            toolchain_file.display()
        )))
    }
}

pub fn read_toolchain_desc_from_file(cfg: &Cfg, toolchain_file: &Path) -> Result<ToolchainDesc> {
    resolve_toolchain_desc(
        cfg,
        &read_unresolved_toolchain_desc_from_file(cfg, toolchain_file)?,
    )
}

impl<'a> Toolchain<'a> {
    pub fn from(cfg: &'a Cfg, desc: &ToolchainDesc) -> Self {
        //We need to replace ":" and "/" with "-" in the toolchain name in order to make a name which is a valid
        //name for a directory.
        let dir_name = desc.to_string().replace("/", "--").replace(":", "---");

        let path = cfg.toolchains_dir.join(&dir_name[..]);

        Toolchain {
            cfg,
            desc: desc.clone(),
            path: path.clone(),
            dist_handler: Box::new(move |n| (cfg.notify_handler)(n.into())),
        }
    }
    pub fn name(&self) -> String {
        self.desc.to_string()
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    fn is_symlink(&self) -> bool {
        use std::fs;
        fs::symlink_metadata(&self.path)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
    }
    pub fn exists(&self) -> bool {
        // HACK: linked toolchains are symlinks, and, contrary to what std docs
        // lead me to believe `fs::metadata`, used by `is_directory` does not
        // seem to follow symlinks on windows.
        utils::is_directory(&self.path) || self.is_symlink()
    }
    pub fn is_custom(&self) -> bool {
        assert!(self.exists());
        self.is_symlink()
    }
    pub fn verify(&self) -> Result<()> {
        Ok(utils::assert_is_directory(&self.path)?)
    }
    pub fn remove(&self) -> Result<()> {
        if self.exists() || self.is_symlink() {
            (self.cfg.notify_handler)(Notification::UninstallingToolchain(&self.desc));
        } else {
            (self.cfg.notify_handler)(Notification::ToolchainNotInstalled(&self.desc));
            return Ok(());
        }
        let result = install::uninstall(&self.path, &|n| (self.cfg.notify_handler)(n.into()));
        if !self.exists() {
            (self.cfg.notify_handler)(Notification::UninstalledToolchain(&self.desc));
        }
        result
    }
    fn install(&self, install_method: InstallMethod<'_>) -> Result<()> {
        let exists = self.exists();
        if exists {
            return Err(format!("'{}' is already installed", self.desc).into());
        } else {
            (self.cfg.notify_handler)(Notification::InstallingToolchain(&self.desc));
        }
        (self.cfg.notify_handler)(Notification::ToolchainDirectory(&self.path, &self.desc));
        install_method.run(&self.path, &|n| (self.cfg.notify_handler)(n.into()))?;

        (self.cfg.notify_handler)(Notification::InstalledToolchain(&self.desc));

        Ok(())
    }
    fn install_if_not_installed(&self, install_method: InstallMethod<'_>) -> Result<()> {
        (self.cfg.notify_handler)(Notification::LookingForToolchain(&self.desc));
        if !self.exists() {
            self.install(install_method)
        } else {
            Ok(())
        }
    }

    fn download_cfg(&self) -> DownloadCfg<'_> {
        DownloadCfg {
            temp_cfg: &self.cfg.temp_cfg,
            notify_handler: &*self.dist_handler,
        }
    }

    pub fn install_from_dist(&self) -> Result<()> {
        self.install(InstallMethod::Dist(&self.desc, self.download_cfg()))
    }

    pub fn install_from_dist_if_not_installed(&self) -> Result<()> {
        self.install_if_not_installed(InstallMethod::Dist(&self.desc, self.download_cfg()))
    }

    pub fn install_from_dir(&self, src: &Path, link: bool) -> Result<()> {
        let mut pathbuf = PathBuf::from(src);

        pathbuf.push("bin");
        utils::assert_is_directory(&pathbuf)?;
        pathbuf.push(format!("lean{}", EXE_SUFFIX));
        utils::assert_is_file(&pathbuf)?;

        if link {
            self.install(InstallMethod::Link(&utils::to_absolute(src)?))?;
        } else {
            self.install(InstallMethod::Copy(src))?;
        }

        Ok(())
    }

    pub fn create_command<T: AsRef<OsStr>>(&self, binary: T) -> Result<Command> {
        self.install_from_dist_if_not_installed()?;

        let bin_path = self.binary_file(&binary);
        let path = if utils::is_file(&bin_path) {
            &bin_path
        } else {
            let recursion_count = env::var("LEAN_RECURSION_COUNT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            if recursion_count > env_var::LEAN_RECURSION_COUNT_MAX - 1 {
                return Err(ErrorKind::BinaryNotFound(
                    self.desc.clone(),
                    bin_path.to_str().unwrap().into(),
                )
                .into());
            }
            Path::new(&binary)
        };
        let mut cmd: Command;
        if cfg!(windows) && path.extension().is_none() {
            cmd = Command::new("sh");
            cmd.arg(format!("'{}'", path.to_str().unwrap()));
        } else {
            cmd = Command::new(path);
        };
        self.set_env(&mut cmd);
        Ok(cmd)
    }

    fn set_env(&self, cmd: &mut Command) {
        self.set_path(cmd);

        env_var::inc("LEAN_RECURSION_COUNT", cmd);

        cmd.env("ELAN_TOOLCHAIN", self.name());
        cmd.env("ELAN_HOME", &self.cfg.elan_dir);
    }

    pub fn set_path(&self, cmd: &mut Command) {
        // Prepend ELAN_HOME/bin to the PATH variable so that we're sure to run
        // lake/lean via the proxy bins. There is no fallback case for if the
        // proxy bins don't exist. We'll just be running whatever happens to
        // be on the PATH.
        let mut path_entries = vec![];
        if let Ok(elan_home) = utils::elan_home() {
            path_entries.push(elan_home.join("bin").to_path_buf());
        }

        if cfg!(target_os = "windows") {
            path_entries.push(self.path.join("bin"));
        }

        env_var::prepend_path("PATH", path_entries, cmd);
    }

    pub fn doc_path(&self, relative: &str) -> Result<PathBuf> {
        self.verify()?;

        let parts = vec!["share", "doc", "lean", "html"];
        let mut doc_dir = self.path.clone();
        for part in parts {
            doc_dir.push(part);
        }
        doc_dir.push(relative);

        Ok(doc_dir)
    }
    pub fn open_docs(&self, relative: &str) -> Result<()> {
        self.verify()?;

        Ok(utils::open_browser(&self.doc_path(relative)?)?)
    }

    pub fn make_override(&self, path: &Path) -> Result<()> {
        self.cfg.settings_file.with_mut(|s| {
            s.add_override(path, self.desc.clone(), self.cfg.notify_handler.as_ref());
            Ok(())
        })
    }

    pub fn binary_file<T: AsRef<OsStr>>(&self, binary: T) -> PathBuf {
        let binary = if let Some(binary_str) = binary.as_ref().to_str() {
            let binary_str = binary_str.to_lowercase();
            let path = Path::new(&binary_str);
            if path.extension().is_some() {
                binary.as_ref().to_owned()
            } else {
                let ext = EXE_SUFFIX;
                OsString::from(format!("{}{}", binary_str, ext))
            }
        } else {
            // Very weird case. Non-unicode command.
            binary.as_ref().to_owned()
        };

        let path = self.path.join("bin").join(&binary);
        if cfg!(windows) && !path.exists() && path.with_extension("bat").exists() {
            // leanpkg.bat
            path.with_extension("bat")
        } else if cfg!(windows) && !path.exists() && path.with_extension("").exists() {
            // leanc (sh script)
            path.with_extension("")
        } else {
            path
        }
    }
}
