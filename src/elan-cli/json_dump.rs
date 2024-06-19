use elan::{lookup_toolchain_desc, utils::{self, fetch_latest_release_tag}, Cfg, Toolchain};
use std::{io, path::PathBuf};

use serde_derive::Serialize;

use elan::OverrideReason;

use crate::common::version;

type Result<T> = std::result::Result<T, String>;

#[derive(Serialize)]
struct Version {
    current: String,
    /// `Err` on network error
    newest: Result<String>,
}

#[derive(Serialize)]
struct InstalledToolchain {
    /// Fully resolved, qualified name, e.g. `leanprover/lean4:v4.9.0`
    resolved_name: String,
    /// Absolute path to toolchain root
    path: PathBuf,
}

#[derive(Serialize)]
struct DefaultToolchain {
    /// Not necessarily resolved name as given to `elan default`, e.g. `stable`
    unresolved: String,
    /// Fully resolved name; `Err` if `unresolved` needed to be resolved but there was a network error
    resolved: Result<String>,
}

#[derive(Serialize)]
struct Toolchains {
    installed: Vec<InstalledToolchain>,
    /// `None` if no default toolchain configured
    default: Option<DefaultToolchain>,
    /// `None` if no override for current directory configured, in which case `default` if any is used
    active_override: Option<OverrideReason>,
    /// Toolchain, if any, ultimately chosen based on `default` and `active_override`
    resolved_active: Option<Result<String>>,
}

#[derive(Serialize)]
pub struct StateDump {
    elan_version: Version,
    toolchains: Toolchains,
}

impl StateDump {
    pub fn new(cfg: &Cfg) -> crate::Result<StateDump> {
        let newest = fetch_latest_release_tag("leanprover/elan", None);
        let ref cwd = utils::current_dir()?;
        let active_override = cfg.find_override(cwd)?;
        let default = cfg.get_default()?;
        Ok(StateDump {
            elan_version: Version {
                current: version().to_string(),
                newest: newest.map_err(|e| e.to_string()),
            },
            toolchains: Toolchains {
                installed: cfg.list_toolchains()?
                    .into_iter()
                    .map(|t| InstalledToolchain {
                        resolved_name: t.to_string(),
                        path: Toolchain::from(cfg, &t).path().to_owned(),
                    }).collect(),
                default: default.as_ref().map(|default| DefaultToolchain {
                    unresolved: default.clone(),
                    resolved: lookup_toolchain_desc(cfg, &default)
                      .map(|t| t.to_string())
                      .map_err(|e| e.to_string()),
                }),
                active_override: active_override.as_ref().map(|p| p.1.clone()),
                resolved_active: active_override
                    .map(|p| p.0.desc.to_string())
                    .or(default)
                    .map(|t| lookup_toolchain_desc(cfg, &t)
                        .map(|tc| tc.to_string())
                        .map_err(|e| e.to_string()))
            },
        })
    }

    pub fn print(&self) -> io::Result<()> {
        serde_json::to_writer_pretty(io::stdout(), self)?;
        Ok(())
    }
}
