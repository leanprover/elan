use std::env;
use std::fmt::{self, Display};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use crate::errors::*;
use crate::notifications::*;
use crate::settings::{Settings, SettingsFile};
use crate::toolchain::Toolchain;
use elan_dist::dist::ToolchainDesc;
use elan_dist::temp;
use elan_utils::utils;
use itertools::Itertools;
use serde_derive::Serialize;

use crate::{
    gc, lookup_toolchain_desc, lookup_unresolved_toolchain_desc,
    read_unresolved_toolchain_desc_from_file, resolve_toolchain_desc, UnresolvedToolchainDesc,
};

#[derive(Debug, Serialize, Clone)]
pub enum OverrideReason {
    /// `ELAN_TOOLCHAIN` environment variable override
    Environment,
    /// `elan override` override
    OverrideDB(PathBuf),
    /// `lean-toolchain` override
    ToolchainFile(PathBuf),
    /// `leanpkg.toml` override lol
    LeanpkgFile(PathBuf),
    /// inside a toolchain directory
    InToolchainDirectory(PathBuf),
}

impl Display for OverrideReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> ::std::result::Result<(), fmt::Error> {
        match *self {
            OverrideReason::Environment => write!(f, "environment override by ELAN_TOOLCHAIN"),
            OverrideReason::OverrideDB(ref path) => {
                write!(f, "directory override for '{}'", path.display())
            }
            OverrideReason::ToolchainFile(ref path) => {
                write!(f, "overridden by '{}'", path.display())
            }
            OverrideReason::InToolchainDirectory(ref path) => {
                write!(
                    f,
                    "override because inside toolchain directory '{}'",
                    path.display()
                )
            }
            OverrideReason::LeanpkgFile(ref path) => {
                write!(f, "overridden by '{}'", path.display())
            }
        }
    }
}

pub struct Cfg {
    pub elan_dir: PathBuf,
    pub settings_file: SettingsFile,
    pub toolchains_dir: PathBuf,
    pub temp_cfg: temp::Cfg,
    //pub gpg_key: Cow<'static, str>,
    pub env_override: Option<String>,
    pub notify_handler: Arc<dyn Fn(Notification<'_>)>,
}

impl Cfg {
    pub fn from_env(notify_handler: Arc<dyn Fn(Notification<'_>)>) -> Result<Self> {
        // Set up the elan home directory
        let elan_dir = utils::elan_home()?;

        utils::ensure_dir_exists("home", &elan_dir, &|n| notify_handler(n.into()))?;

        let settings_file = SettingsFile::new(elan_dir.join("settings.toml"));

        let toolchains_dir = elan_dir.join("toolchains");

        // GPG key
        /*let gpg_key = ""; if let Some(path) = env::var_os("ELAN_GPG_KEY")
                                              .and_then(utils::if_not_empty) {
            Cow::Owned(try!(utils::read_file("public key", Path::new(&path))))
        } else {
            Cow::Borrowed(include_str!("lean-key.gpg.ascii"))
        };*/

        // Environment override
        let env_override = env::var("ELAN_TOOLCHAIN")
            .ok()
            .and_then(utils::if_not_empty);

        let notify_clone = notify_handler.clone();
        let temp_cfg = temp::Cfg::new(
            elan_dir.join("tmp"),
            Box::new(move |n| (notify_clone)(n.into())),
        );

        Ok(Cfg {
            elan_dir,
            settings_file,
            toolchains_dir,
            temp_cfg,
            //gpg_key: gpg_key,
            notify_handler,
            env_override,
        })
    }

    pub fn set_default(&self, toolchain: &str) -> Result<()> {
        self.settings_file.with_mut(|s| {
            s.default_toolchain = Some(toolchain.to_owned());
            Ok(())
        })?;
        (self.notify_handler)(Notification::SetDefaultToolchain(toolchain));
        Ok(())
    }

    pub fn get_toolchain(
        &self,
        name: &ToolchainDesc,
        create_parent: bool,
    ) -> Result<Toolchain<'_>> {
        if create_parent {
            utils::ensure_dir_exists("toolchains", &self.toolchains_dir, &|n| {
                (self.notify_handler)(n.into())
            })?;
        }

        Ok(Toolchain::from(self, name))
    }

    pub fn which_binary(&self, path: &Path, binary: &str) -> Result<Option<PathBuf>> {
        if let Some((toolchain, _)) = self.find_override_toolchain_or_default(path)? {
            Ok(Some(toolchain.binary_file(binary)))
        } else {
            Ok(None)
        }
    }

    pub fn get_default(&self) -> Result<Option<String>> {
        self.settings_file.with(|s| Ok(s.default_toolchain.clone()))
    }

    pub fn resolve_default(&self) -> Result<Option<ToolchainDesc>> {
        if let Some(name) = self.get_default()? {
            let toolchain = lookup_toolchain_desc(self, &name)?;
            Ok(Some(toolchain))
        } else {
            Ok(None)
        }
    }

    pub fn find_override(
        &self,
        path: &Path,
    ) -> Result<Option<(UnresolvedToolchainDesc, OverrideReason)>> {
        // First check ELAN_TOOLCHAIN
        if let Some(ref name) = self.env_override {
            return Ok(Some((
                lookup_unresolved_toolchain_desc(self, name)?,
                OverrideReason::Environment,
            )));
        }

        // Then walk up the directory tree from 'path' looking for either the
        // directory in override database, a `lean-toolchain` file, or a
        // `leanpkg.toml` file.
        if let Some(res) = self
            .settings_file
            .with(|s| self.find_override_from_dir_walk(path, s))?
        {
            return Ok(Some(res));
        }
        Ok(None)
    }

    fn find_override_from_dir_walk(
        &self,
        dir: &Path,
        settings: &Settings,
    ) -> Result<Option<(UnresolvedToolchainDesc, OverrideReason)>> {
        let notify = self.notify_handler.as_ref();
        let dir = utils::canonicalize_path(dir, &|n| notify(n.into()));
        let mut dir = Some(&*dir);

        while let Some(d) = dir {
            // First check the override database
            if let Some(name) = settings.dir_override(d, notify) {
                let reason = OverrideReason::OverrideDB(d.to_owned());
                return Ok(Some((UnresolvedToolchainDesc(name), reason)));
            }

            // Then look for 'lean-toolchain'
            let toolchain_file = d.join("lean-toolchain");
            if let Ok(desc) = read_unresolved_toolchain_desc_from_file(self, &toolchain_file) {
                let reason = OverrideReason::ToolchainFile(toolchain_file);
                gc::add_root(self, d)?;
                return Ok(Some((desc, reason)));
            }

            // Then look for 'leanpkg.toml'
            let leanpkg_file = d.join("leanpkg.toml");
            if let Ok(content) = utils::read_file("leanpkg.toml", &leanpkg_file) {
                let value = content
                    .parse::<toml::Value>()
                    .map_err(|error| ErrorKind::InvalidLeanpkgFile(leanpkg_file.clone(), error))?;
                match value
                    .get("package")
                    .and_then(|package| package.get("lean_version"))
                {
                    None => {}
                    Some(toml::Value::String(s)) => {
                        let desc = lookup_unresolved_toolchain_desc(self, s)?;
                        return Ok(Some((desc, OverrideReason::LeanpkgFile(leanpkg_file))));
                    }
                    Some(a) => {
                        return Err(ErrorKind::InvalidLeanVersion(leanpkg_file, a.type_str()).into())
                    }
                }
            }

            dir = d.parent();

            if dir == Some(&self.toolchains_dir) {
                if let Some(last) = d.file_name() {
                    if let Some(last) = last.to_str() {
                        return Ok(Some((
                            UnresolvedToolchainDesc(ToolchainDesc::from_toolchain_dir(last)?),
                            OverrideReason::InToolchainDirectory(d.into()),
                        )));
                    }
                }
            }
        }

        Ok(None)
    }

    pub fn find_override_toolchain_or_default(
        &self,
        path: &Path,
    ) -> Result<Option<(Toolchain<'_>, Option<OverrideReason>)>> {
        if let Some((toolchain, reason)) = self.find_override(path)? {
            let toolchain = resolve_toolchain_desc(self, &toolchain)?;
            match self.get_toolchain(&toolchain, false) {
                Ok(toolchain) => {
                    if toolchain.exists() {
                        Ok(Some((toolchain, Some(reason))))
                    } else {
                        toolchain.install_from_dist()?;
                        Ok(Some((toolchain, Some(reason))))
                    }
                }
                Err(e) => {
                    // This is hackishly using the error chain to provide a bit of
                    // extra context about what went wrong. The CLI will display it
                    // on a line after the proximate error.

                    let reason_err = match reason {
                        OverrideReason::Environment => {
                            "the ELAN_TOOLCHAIN environment variable specifies an uninstalled toolchain"
                                .to_string()
                        }
                        OverrideReason::OverrideDB(ref path) => {
                            format!(
                                "the directory override for '{}' specifies an uninstalled toolchain",
                                path.display()
                            )
                        }
                        OverrideReason::ToolchainFile(ref path) => {
                            format!(
                                "the toolchain file at '{}' specifies an uninstalled toolchain",
                                path.display()
                            )
                        }
                        OverrideReason::LeanpkgFile(ref path) => {
                            format!(
                                "the leanpkg.toml file at '{}' specifies an uninstalled toolchain",
                                path.display()
                            )
                        }
                        OverrideReason::InToolchainDirectory(ref path) => {
                            format!(
                                "could not parse toolchain directory at '{}'",
                                path.display()
                            )
                        }
                    };
                    Err(e)
                        .chain_err(|| Error::from(reason_err))
                        .chain_err(|| ErrorKind::OverrideToolchainNotInstalled(toolchain))
                }
            }
        } else if let Some(tc) = self.resolve_default()? {
            Ok(Some((self.get_toolchain(&tc, false)?, None)))
        } else {
            Ok(None)
        }
    }

    pub fn get_overrides(&self) -> Result<Vec<(String, ToolchainDesc)>> {
        self.settings_file
            .with(|s| Ok(s.overrides.clone().into_iter().collect_vec()))
    }

    pub fn list_toolchains(&self) -> Result<Vec<ToolchainDesc>> {
        if utils::is_directory(&self.toolchains_dir) {
            let mut toolchains: Vec<_> = utils::read_dir("toolchains", &self.toolchains_dir)?
                .filter_map(io::Result::ok)
                .filter(|e| e.file_type().map(|f| !f.is_file()).unwrap_or(false))
                .filter_map(|e| e.file_name().into_string().ok())
                .map(|n| ToolchainDesc::from_toolchain_dir(&n).map_err(|e| e.into()))
                .collect::<Result<Vec<ToolchainDesc>>>()?
                .into_iter()
                .map(|tc| tc.to_string())
                .collect();

            utils::toolchain_sort(&mut toolchains);

            let toolchains: Vec<_> = toolchains
                .iter()
                .flat_map(|s| ToolchainDesc::from_resolved_str(s))
                .collect();
            Ok(toolchains)
        } else {
            Ok(Vec::new())
        }
    }

    pub fn toolchain_for_dir(
        &self,
        path: &Path,
    ) -> Result<(Toolchain<'_>, Option<OverrideReason>)> {
        self.find_override_toolchain_or_default(path)
            .and_then(|r| r.ok_or(ErrorKind::NoDefaultToolchain.into()))
    }

    pub fn create_command_for_dir(&self, path: &Path, binary: &str) -> Result<Command> {
        let (ref toolchain, _) = self.toolchain_for_dir(path)?;

        toolchain.create_command(binary)
    }

    pub fn create_command_for_toolchain(
        &self,
        toolchain: &ToolchainDesc,
        install_if_missing: bool,
        binary: &str,
    ) -> Result<Command> {
        let toolchain = &(self.get_toolchain(toolchain, false)?);
        if install_if_missing && !toolchain.exists() {
            toolchain.install_from_dist()?;
        }

        toolchain.create_command(binary)
    }

    pub fn doc_path_for_dir(&self, path: &Path, relative: &str) -> Result<PathBuf> {
        let (toolchain, _) = self.toolchain_for_dir(path)?;
        toolchain.doc_path(relative)
    }

    pub fn open_docs_for_dir(&self, path: &Path, relative: &str) -> Result<()> {
        let (toolchain, _) = self.toolchain_for_dir(path)?;
        toolchain.open_docs(relative)
    }
}
