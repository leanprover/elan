use elan_dist::dist::ToolchainDesc;
use errors::*;
use notifications::*;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use toml;
use toml_utils::*;
use utils;

pub const SUPPORTED_METADATA_VERSIONS: [&'static str; 2] = ["2", "12"];
pub const DEFAULT_METADATA_VERSION: &'static str = "12";

#[derive(Clone, Debug, PartialEq)]
pub struct SettingsFile {
    path: PathBuf,
    cache: RefCell<Option<Settings>>,
}

impl SettingsFile {
    pub fn new(path: PathBuf) -> Self {
        SettingsFile {
            path: path,
            cache: RefCell::new(None),
        }
    }
    fn write_settings(&self) -> Result<()> {
        let s = self.cache.borrow().as_ref().unwrap().clone();
        utils::write_file("settings", &self.path, &s.stringify())?;
        Ok(())
    }
    fn read_settings(&self) -> Result<()> {
        let mut needs_save = false;
        {
            let mut b = self.cache.borrow_mut();
            if b.is_none() {
                *b = Some(if utils::is_file(&self.path) {
                    let content = utils::read_file("settings", &self.path)?;
                    Settings::parse(&content)?
                } else {
                    needs_save = true;
                    Default::default()
                });
            }
        }
        if needs_save {
            self.write_settings()?;
        }
        Ok(())
    }
    pub fn with<T, F: FnOnce(&Settings) -> Result<T>>(&self, f: F) -> Result<T> {
        self.read_settings()?;

        // Settings can no longer be None so it's OK to unwrap
        f(self.cache.borrow().as_ref().unwrap())
    }
    pub fn with_mut<T, F: FnOnce(&mut Settings) -> Result<T>>(&self, f: F) -> Result<T> {
        self.read_settings()?;

        // Settings can no longer be None so it's OK to unwrap
        let result = { f(self.cache.borrow_mut().as_mut().unwrap())? };
        self.write_settings()?;
        Ok(result)
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TelemetryMode {
    On,
    Off,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub version: String,
    pub default_toolchain: Option<ToolchainDesc>,
    pub overrides: BTreeMap<String, ToolchainDesc>,
    pub telemetry: TelemetryMode,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            version: DEFAULT_METADATA_VERSION.to_owned(),
            default_toolchain: None,
            overrides: BTreeMap::new(),
            telemetry: TelemetryMode::Off,
        }
    }
}

impl Settings {
    fn path_to_key(path: &Path, notify_handler: &dyn Fn(Notification)) -> String {
        if path.exists() {
            utils::canonicalize_path(path, &|n| notify_handler(n.into()))
                .display()
                .to_string()
        } else {
            path.display().to_string()
        }
    }

    pub fn remove_override(&mut self, path: &Path, notify_handler: &dyn Fn(Notification)) -> bool {
        let key = Self::path_to_key(path, notify_handler);
        self.overrides.remove(&key).is_some()
    }

    pub fn add_override(
        &mut self,
        path: &Path,
        toolchain: ToolchainDesc,
        notify_handler: &dyn Fn(Notification),
    ) {
        let key = Self::path_to_key(path, notify_handler);
        notify_handler(Notification::SetOverrideToolchain(path, &toolchain));
        self.overrides.insert(key, toolchain);
    }

    pub fn dir_override(
        &self,
        dir: &Path,
        notify_handler: &dyn Fn(Notification),
    ) -> Option<ToolchainDesc> {
        let key = Self::path_to_key(dir, notify_handler);
        self.overrides.get(&key).map(|s| s.clone())
    }

    pub fn parse(data: &str) -> Result<Self> {
        let value = toml::from_str(data).map_err(ErrorKind::ParsingSettings)?;
        Self::from_toml(value, "")
    }
    pub fn stringify(self) -> String {
        toml::Value::Table(self.to_toml()).to_string()
    }

    pub fn from_toml(mut table: toml::value::Table, path: &str) -> Result<Self> {
        let version = get_string(&mut table, "version", path)?;
        if !SUPPORTED_METADATA_VERSIONS.contains(&&*version) {
            return Err(ErrorKind::UnknownMetadataVersion(version).into());
        }
        Ok(Settings {
            version: version,
            default_toolchain: get_opt_string(&mut table, "default_toolchain", path)?
                .map(|s| ToolchainDesc::from_str(&s))
                .map_or(Ok(None), |r| r.map(Some))?,
            overrides: Self::table_to_overrides(&mut table, path)?,
            telemetry: if get_opt_bool(&mut table, "telemetry", path)?.unwrap_or(false) {
                TelemetryMode::On
            } else {
                TelemetryMode::Off
            },
        })
    }
    pub fn to_toml(self) -> toml::value::Table {
        let mut result = toml::value::Table::new();

        result.insert("version".to_owned(), toml::Value::String(self.version));

        if let Some(v) = self.default_toolchain {
            result.insert(
                "default_toolchain".to_owned(),
                toml::Value::String(v.to_string()),
            );
        }

        let overrides = Self::overrides_to_table(self.overrides);
        result.insert("overrides".to_owned(), toml::Value::Table(overrides));

        let telemetry = self.telemetry == TelemetryMode::On;
        result.insert("telemetry".to_owned(), toml::Value::Boolean(telemetry));

        result
    }

    fn table_to_overrides(
        table: &mut toml::value::Table,
        path: &str,
    ) -> Result<BTreeMap<String, ToolchainDesc>> {
        let mut result = BTreeMap::new();
        let pkg_table = get_table(table, "overrides", path)?;

        for (k, v) in pkg_table {
            if let toml::Value::String(t) = v {
                result.insert(k, ToolchainDesc::from_str(&t)?);
            }
        }

        Ok(result)
    }

    fn overrides_to_table(overrides: BTreeMap<String, ToolchainDesc>) -> toml::value::Table {
        let mut result = toml::value::Table::new();
        for (k, v) in overrides {
            result.insert(k, toml::Value::String(v.to_string()));
        }
        result
    }
}
