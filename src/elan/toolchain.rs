use config::Cfg;
use elan_dist;
use elan_dist::dist::ToolchainDesc;
use elan_dist::download::DownloadCfg;
use elan_dist::manifest::Component;
use elan_utils::utils;
use env_var;
use errors::*;
use install::{self, InstallMethod};
use notifications::*;
use telemetry;
use telemetry::{Telemetry, TelemetryEvent};

use std::env;
use std::env::consts::EXE_SUFFIX;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use regex::Regex;

/// A fully resolved reference to a toolchain which may or may not exist
pub struct Toolchain<'a> {
    cfg: &'a Cfg,
    name: String,
    dir_name: String,
    path: PathBuf,
    telemetry: telemetry::Telemetry,
    dist_handler: Box<Fn(elan_dist::Notification) + 'a>,
}

/// Used by the `list_component` function
pub struct ComponentStatus {
    pub component: Component,
    pub required: bool,
    pub installed: bool,
    pub available: bool,
}

pub enum UpdateStatus {
    Installed,
    Updated,
    Unchanged,
}

impl<'a> Toolchain<'a> {
    pub fn from(cfg: &'a Cfg, name: &str) -> Result<Self> {
        //We need to replace ":" and "/" with "-" in the toolchain name in order to make a name which is a valid
        //name for a directory.
        let dir_name = name.replace("/", "--").replace(":", "---");

        let path = cfg.toolchains_dir.join(&dir_name[..]);

        Ok(Toolchain {
            cfg: cfg,
            name: name.to_owned(),
            dir_name: dir_name,
            path: path.clone(),
            telemetry: Telemetry::new(cfg.elan_dir.join("telemetry")),
            dist_handler: Box::new(move |n| (cfg.notify_handler)(n.into())),
        })
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn desc(&self) -> Result<ToolchainDesc> {
        Ok(ToolchainDesc::from_str(&self.name)?)
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
            (self.cfg.notify_handler)(Notification::UninstallingToolchain(&self.name));
        } else {
            (self.cfg.notify_handler)(Notification::ToolchainNotInstalled(&self.name));
            return Ok(());
        }
        if let Some(update_hash) = self.update_hash()? {
            utils::remove_file("update hash", &update_hash)?;
        }
        let result = install::uninstall(&self.path, &|n| (self.cfg.notify_handler)(n.into()));
        if !self.exists() {
            (self.cfg.notify_handler)(Notification::UninstalledToolchain(&self.name));
        }
        Ok(result?)
    }
    fn install(&self, install_method: InstallMethod) -> Result<UpdateStatus> {
        let exists = self.exists();
        if exists {
            (self.cfg.notify_handler)(Notification::UpdatingToolchain(&self.name));
        } else {
            (self.cfg.notify_handler)(Notification::InstallingToolchain(&self.name));
        }
        (self.cfg.notify_handler)(Notification::ToolchainDirectory(&self.path, &self.name));
        let updated = install_method.run(&self.path, &|n| (self.cfg.notify_handler)(n.into()))?;

        if !updated {
            (self.cfg.notify_handler)(Notification::UpdateHashMatches);
        } else {
            (self.cfg.notify_handler)(Notification::InstalledToolchain(&self.name));
        }

        let status = match (updated, exists) {
            (true, false) => UpdateStatus::Installed,
            (true, true) => UpdateStatus::Updated,
            (false, true) => UpdateStatus::Unchanged,
            (false, false) => UpdateStatus::Unchanged,
        };

        Ok(status)
    }
    fn install_if_not_installed(&self, install_method: InstallMethod) -> Result<UpdateStatus> {
        (self.cfg.notify_handler)(Notification::LookingForToolchain(&self.name));
        if !self.exists() {
            Ok(self.install(install_method)?)
        } else {
            (self.cfg.notify_handler)(Notification::UsingExistingToolchain(&self.name));
            Ok(UpdateStatus::Unchanged)
        }
    }
    fn update_hash(&self) -> Result<Option<PathBuf>> {
        if self.is_symlink() {
            Ok(None)
        } else {
            Ok(Some(self.cfg.get_hash_file(&self.dir_name, true)?))
        }
    }

    fn download_cfg(&self) -> DownloadCfg {
        DownloadCfg {
            temp_cfg: &self.cfg.temp_cfg,
            download_dir: &self.cfg.download_dir,
            notify_handler: &*self.dist_handler,
        }
    }

    pub fn install_from_dist(&self, force_update: bool) -> Result<UpdateStatus> {
        if self.cfg.telemetry_enabled()? {
            return self.install_from_dist_with_telemetry(force_update);
        }
        self.install_from_dist_inner(force_update)
    }

    pub fn install_from_dist_inner(&self, force_update: bool) -> Result<UpdateStatus> {
        let update_hash = self.update_hash()?;
        self.install(InstallMethod::Dist(
            &self.desc()?,
            update_hash.as_ref().map(|p| &**p),
            self.download_cfg(),
            force_update,
        ))
    }

    pub fn install_from_dist_with_telemetry(&self, force_update: bool) -> Result<UpdateStatus> {
        let result = self.install_from_dist_inner(force_update);

        match result {
            Ok(us) => {
                let te = TelemetryEvent::ToolchainUpdate {
                    toolchain: self.name().to_string(),
                    success: true,
                };
                match self.telemetry.log_telemetry(te) {
                    Ok(_) => Ok(us),
                    Err(e) => {
                        (self.cfg.notify_handler)(Notification::TelemetryCleanupError(&e));
                        Ok(us)
                    }
                }
            }
            Err(e) => {
                let te = TelemetryEvent::ToolchainUpdate {
                    toolchain: self.name().to_string(),
                    success: true,
                };
                let _ = self.telemetry.log_telemetry(te).map_err(|xe| {
                    (self.cfg.notify_handler)(Notification::TelemetryCleanupError(&xe));
                });
                Err(e)
            }
        }
    }

    pub fn install_from_dist_if_not_installed(&self) -> Result<UpdateStatus> {
        let update_hash = self.update_hash()?;
        self.install_if_not_installed(InstallMethod::Dist(
            &self.desc()?,
            update_hash.as_ref().map(|p| &**p),
            self.download_cfg(),
            false,
        ))
    }
    pub fn is_tracking(&self) -> bool {
        ToolchainDesc::from_str(&self.name)
            .ok()
            .map(|d| d.is_tracking())
            == Some(true)
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
        if !self.exists() {
            return Err(ErrorKind::ToolchainNotInstalled(self.name.to_owned()).into());
        }

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
                    self.name.clone(),
                    bin_path.to_str().unwrap().into(),
                )
                .into());
            }
            Path::new(&binary)
        };
        let mut cmd: Command;
        if cfg!(windows) && path.extension() == None {
            cmd = Command::new("sh");
            cmd.arg(format!("'{}'", path.to_str().unwrap()));
        } else {
            cmd = Command::new(&path);
        };
        self.set_env(&mut cmd);
        Ok(cmd)
    }

    fn set_env(&self, cmd: &mut Command) {
        self.set_ldpath(cmd);

        // Because elan and leanpkg use slightly different
        // definitions of leanpkg home (elan doesn't read HOME on
        // windows), we must set it here to ensure leanpkg and
        // elan agree.
        if let Ok(elan_home) = utils::elan_home() {
            cmd.env("ELAN_HOME", &elan_home);
        }

        env_var::inc("LEAN_RECURSION_COUNT", cmd);

        cmd.env("ELAN_TOOLCHAIN", &self.name);
        cmd.env("ELAN_HOME", &self.cfg.elan_dir);
    }

    pub fn set_ldpath(&self, cmd: &mut Command) {
        let new_path = self.path.join("lib");

        #[cfg(not(target_os = "macos"))]
        mod sysenv {
            pub const LOADER_PATH: &'static str = "LD_LIBRARY_PATH";
        }
        #[cfg(target_os = "macos")]
        mod sysenv {
            pub const LOADER_PATH: &'static str = "DYLD_LIBRARY_PATH";
        }
        env_var::prepend_path(sysenv::LOADER_PATH, vec![new_path.clone()], cmd);

        // Prepend ELAN_HOME/bin to the PATH variable so that we're sure to run
        // leanpkg/lean via the proxy bins. There is no fallback case for if the
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

    pub fn make_default(&self) -> Result<()> {
        self.cfg.set_default(&self.name)
    }
    pub fn make_override(&self, path: &Path) -> Result<()> {
        Ok(self.cfg.settings_file.with_mut(|s| {
            s.add_override(path, self.name.clone(), self.cfg.notify_handler.as_ref());
            Ok(())
        })?)
    }

    pub fn binary_file<T: AsRef<OsStr>>(&self, binary: T) -> PathBuf {
        let binary = if let Some(binary_str) = binary.as_ref().to_str() {
            let binary_str = binary_str.to_lowercase();
            let path = Path::new(&binary_str);
            if path.extension().is_some() {
                binary.as_ref().to_owned()
            } else {
                let mut ext = EXE_SUFFIX;
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
