use std::path::{Path, PathBuf};
use std::env;
use std::io;
use std::process::Command;
use std::fmt::{self, Display};
use std::sync::Arc;

use errors::*;
use notifications::*;
use elan_dist::{temp};
use elan_utils::utils;
use toolchain::{Toolchain, UpdateStatus};
use telemetry_analysis::*;
use settings::{TelemetryMode, SettingsFile, Settings};

use toml;

#[derive(Debug)]
pub enum OverrideReason {
    Environment,
    OverrideDB(PathBuf),
    ToolchainFile(PathBuf),
    LeanpkgFile(PathBuf),
}

impl Display for OverrideReason {
    fn fmt(&self, f: &mut fmt::Formatter) -> ::std::result::Result<(), fmt::Error> {
        match *self {
            OverrideReason::Environment => write!(f, "environment override by ELAN_TOOLCHAIN"),
            OverrideReason::OverrideDB(ref path) => {
                write!(f, "directory override for '{}'", path.display())
            }
            OverrideReason::ToolchainFile(ref path) => {
                write!(f, "overridden by '{}'", path.display())
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
    pub update_hash_dir: PathBuf,
    pub download_dir: PathBuf,
    pub temp_cfg: temp::Cfg,
    //pub gpg_key: Cow<'static, str>,
    pub env_override: Option<String>,
    pub notify_handler: Arc<Fn(Notification)>,
}

impl Cfg {
    pub fn from_env(notify_handler: Arc<Fn(Notification)>) -> Result<Self> {
        // Set up the elan home directory
        let elan_dir = try!(utils::elan_home());

        try!(utils::ensure_dir_exists("home", &elan_dir,
                                      &|n| notify_handler(n.into())));

        let settings_file = SettingsFile::new(elan_dir.join("settings.toml"));

        let toolchains_dir = elan_dir.join("toolchains");
        let update_hash_dir = elan_dir.join("update-hashes");
        let download_dir = elan_dir.join("downloads");

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
        let temp_cfg = temp::Cfg::new(elan_dir.join("tmp"),
                                      Box::new(move |n| {
                                          (notify_clone)(n.into())
                                      }));

        Ok(Cfg {
            elan_dir: elan_dir,
            settings_file: settings_file,
            toolchains_dir: toolchains_dir,
            update_hash_dir: update_hash_dir,
            download_dir: download_dir,
            temp_cfg: temp_cfg,
            //gpg_key: gpg_key,
            notify_handler: notify_handler,
            env_override: env_override,
        })
    }

    pub fn set_default(&self, toolchain: &str) -> Result<()> {
        try!(self.settings_file.with_mut(|s| {
            s.default_toolchain = Some(toolchain.to_owned());
            Ok(())
        }));
        (self.notify_handler)(Notification::SetDefaultToolchain(toolchain));
        Ok(())
    }

    pub fn get_toolchain(&self, name: &str, create_parent: bool) -> Result<Toolchain> {
        if create_parent {
            try!(utils::ensure_dir_exists("toolchains",
                                          &self.toolchains_dir,
                                          &|n| (self.notify_handler)(n.into())));
        }

        Toolchain::from(self, name)
    }

    pub fn verify_toolchain(&self, name: &str) -> Result<Toolchain> {
        let toolchain = try!(self.get_toolchain(name, false));
        try!(toolchain.verify());
        Ok(toolchain)
    }

    pub fn get_hash_file(&self, toolchain: &str, create_parent: bool) -> Result<PathBuf> {
        if create_parent {
            try!(utils::ensure_dir_exists("update-hash",
                                          &self.update_hash_dir,
                                          &|n| (self.notify_handler)(n.into())));
        }

        Ok(self.update_hash_dir.join(toolchain))
    }

    pub fn which_binary(&self, path: &Path, binary: &str) -> Result<Option<PathBuf>> {

        if let Some((toolchain, _)) = try!(self.find_override_toolchain_or_default(path)) {
            Ok(Some(toolchain.binary_file(binary)))
        } else {
            Ok(None)
        }
    }

    pub fn find_default(&self) -> Result<Option<Toolchain>> {
        let opt_name = try!(self.settings_file.with(|s| Ok(s.default_toolchain.clone())));

        if let Some(name) = opt_name {
            let toolchain = try!(self.verify_toolchain(&name)
                                 .chain_err(|| ErrorKind::ToolchainNotInstalled(name.to_string())));

            Ok(Some(toolchain))
        } else {
            Ok(None)
        }
    }

    pub fn find_override(&self, path: &Path) -> Result<Option<(Toolchain, OverrideReason)>> {
        let mut override_ = None;

        // First check ELAN_TOOLCHAIN
        if let Some(ref name) = self.env_override {
            override_ = Some((name.to_string(), OverrideReason::Environment));
        }

        // Then walk up the directory tree from 'path' looking for either the
        // directory in override database, a `lean-toolchain` file, or a
        // `leanpkg.toml` file.
        if override_.is_none() {
            self.settings_file.with(|s| {
                override_ = self.find_override_from_dir_walk(path, s)?;

                Ok(())
            })?;
        }

        if let Some((name, reason)) = override_ {
            // This is hackishly using the error chain to provide a bit of
            // extra context about what went wrong. The CLI will display it
            // on a line after the proximate error.

            let reason_err = match reason {
                OverrideReason::Environment => {
                    format!("the ELAN_TOOLCHAIN environment variable specifies an uninstalled toolchain")
                }
                OverrideReason::OverrideDB(ref path) => {
                    format!("the directory override for '{}' specifies an uninstalled toolchain", path.display())
                }
                OverrideReason::ToolchainFile(ref path) => {
                    format!("the toolchain file at '{}' specifies an uninstalled toolchain", path.display())
                }
                OverrideReason::LeanpkgFile(ref path) => {
                    format!("the leanpkg.toml file at '{}' specifies an uninstalled toolchain", path.display())
                }
            };

            match self.get_toolchain(&name, false) {
                Ok(toolchain) => {
                    if toolchain.exists() {
                        Ok(Some((toolchain, reason)))
                    } else {
                        try!(toolchain.install_from_dist(false));
                        Ok(Some((toolchain, reason)))
                    }
                }
                Err(e) => {
                    Err(e)
                        .chain_err(|| Error::from(reason_err))
                        .chain_err(|| ErrorKind::OverrideToolchainNotInstalled(name.to_string()))
                }
            }
        } else {
            Ok(None)
        }
    }

    fn find_override_from_dir_walk(&self, dir: &Path, settings: &Settings)
                                   -> Result<Option<(String, OverrideReason)>>
    {
        let notify = self.notify_handler.as_ref();
        let dir = utils::canonicalize_path(dir, &|n| notify(n.into()));
        let mut dir = Some(&*dir);

        while let Some(d) = dir {
            // First check the override database
            if let Some(name) = settings.dir_override(d, notify) {
                let reason = OverrideReason::OverrideDB(d.to_owned());
                return Ok(Some((name, reason)));
            }

            // Then look for 'lean-toolchain'
            let toolchain_file = d.join("lean-toolchain");
            if let Ok(s) = utils::read_file("toolchain file", &toolchain_file) {
                if let Some(s) = s.lines().next() {
                    let toolchain_name = s.trim();
                    let reason = OverrideReason::ToolchainFile(toolchain_file);
                    return Ok(Some((toolchain_name.to_string(), reason)));
                }
            }

            // Then look for 'leanpkg.toml'
            let leanpkg_file = d.join("leanpkg.toml");
            if let Ok(content) = utils::read_file("leanpkg.toml", &leanpkg_file) {
                let value = content.parse::<toml::Value>()
                    .map_err(|error| ErrorKind::InvalidLeanpkgFile(leanpkg_file.clone(), error))?;
                match value.get("package").and_then(|package| package.get("lean_version")) {
                    None => {}
                    Some(toml::Value::String(s)) => {
                        return Ok(Some((s.to_string(), OverrideReason::LeanpkgFile(leanpkg_file))))
                    }
                    Some(a) => {
                        return Err(ErrorKind::InvalidLeanVersion(leanpkg_file, a.type_str()).into())
                    }
                }
            }

            dir = d.parent();
        }

        Ok(None)
    }

    pub fn find_override_toolchain_or_default
        (&self,
         path: &Path)
         -> Result<Option<(Toolchain, Option<OverrideReason>)>> {
        Ok(if let Some((toolchain, reason)) = try!(self.find_override(path)) {
            Some((toolchain, Some(reason)))
        } else {
            try!(self.find_default()).map(|toolchain| (toolchain, None))
        })
    }

    pub fn get_default(&self) -> Result<String> {
        self.settings_file.with(|s| { 
            Ok(s.default_toolchain.clone().unwrap())
        })
    }

    pub fn list_toolchains(&self) -> Result<Vec<String>> {
        if utils::is_directory(&self.toolchains_dir) {
            let mut toolchains: Vec<_> = try!(utils::read_dir("toolchains", &self.toolchains_dir))
                                         .filter_map(io::Result::ok)
                                         .filter(|e| e.file_type().map(|f| !f.is_file()).unwrap_or(false))
                                         .filter_map(|e| e.file_name().into_string().ok())
                                         .collect();

            utils::toolchain_sort(&mut toolchains);

            Ok(toolchains)
        } else {
            Ok(Vec::new())
        }
    }

    pub fn update_all_channels(&self, force_update: bool) -> Result<Vec<(String, Result<UpdateStatus>)>> {
        let toolchains = try!(self.list_toolchains());

        // Convert the toolchain strings to Toolchain values
        let toolchains = toolchains.into_iter();
        let toolchains = toolchains.map(|n| (n.clone(), self.get_toolchain(&n, true)));

        // Filter out toolchains that don't track a release channel
        let toolchains = toolchains.filter(|&(_, ref t)| {
            t.as_ref().map(|t| t.is_tracking()).unwrap_or(false)
        });

        // Update toolchains and collect the results
        let toolchains = toolchains.map(|(n, t)| {
            let t = t.and_then(|t| {
                let t = t.install_from_dist(force_update);
                if let Err(ref e) = t {
                    (self.notify_handler)(Notification::NonFatalError(e));
                }
                t
            });

            (n, t)
        });

        Ok(toolchains.collect())
    }

    pub fn toolchain_for_dir(&self, path: &Path) -> Result<(Toolchain, Option<OverrideReason>)> {
        self.find_override_toolchain_or_default(path)
            .and_then(|r| r.ok_or(ErrorKind::NoDefaultToolchain.into()))
    }

    pub fn create_command_for_dir(&self, path: &Path, binary: &str) -> Result<Command> {
        let (ref toolchain, _) = try!(self.toolchain_for_dir(path));

        toolchain.create_command(binary)
    }

    pub fn create_command_for_toolchain(&self, toolchain: &str, install_if_missing: bool,
                                        binary: &str) -> Result<Command> {
        let ref toolchain = try!(self.get_toolchain(toolchain, false));
        if install_if_missing && !toolchain.exists() {
            try!(toolchain.install_from_dist(false));
        }

        toolchain.create_command(binary)
    }

    pub fn doc_path_for_dir(&self, path: &Path, relative: &str) -> Result<PathBuf> {
        let (toolchain, _) = try!(self.toolchain_for_dir(path));
        toolchain.doc_path(relative)
    }

    pub fn open_docs_for_dir(&self, path: &Path, relative: &str) -> Result<()> {
        let (toolchain, _) = try!(self.toolchain_for_dir(path));
        toolchain.open_docs(relative)
    }

    pub fn set_telemetry(&self, telemetry_enabled: bool) -> Result<()> {
        if telemetry_enabled { self.enable_telemetry() } else { self.disable_telemetry() }
    }

    fn enable_telemetry(&self) -> Result<()> {
        try!(self.settings_file.with_mut(|s| {
            s.telemetry = TelemetryMode::On;
            Ok(())
        }));

        let _ = utils::ensure_dir_exists("telemetry", &self.elan_dir.join("telemetry"),
                                         &|_| ());

        (self.notify_handler)(Notification::SetTelemetry("on"));

        Ok(())
    }

    fn disable_telemetry(&self) -> Result<()> {
        try!(self.settings_file.with_mut(|s| {
            s.telemetry = TelemetryMode::Off;
            Ok(())
        }));

        (self.notify_handler)(Notification::SetTelemetry("off"));

        Ok(())
    }

    pub fn telemetry_enabled(&self) -> Result<bool> {
        Ok(match try!(self.settings_file.with(|s| Ok(s.telemetry))) {
            TelemetryMode::On => true,
            TelemetryMode::Off => false,
        })
    }

    pub fn analyze_telemetry(&self) -> Result<TelemetryAnalysis> {
        let mut t = TelemetryAnalysis::new(self.elan_dir.join("telemetry"));

        let events = try!(t.import_telemery());
        try!(t.analyze_telemetry_events(&events));

        Ok(t)
    }
}
