use elan::{lookup_unresolved_toolchain_desc, resolve_toolchain_desc, utils::{self, fetch_latest_release_tag}, Cfg, Toolchain, UnresolvedToolchainDesc};
use std::{io, path::PathBuf};

use serde_derive::Serialize;

use elan::OverrideReason;

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
    unresolved: UnresolvedToolchainDesc,
    /// Fully resolved name; `Err` if `unresolved` needed to be resolved but there was a network error
    resolved: Result<String>,
}

#[derive(Serialize)]
struct Override {
    unresolved: UnresolvedToolchainDesc,
    reason: OverrideReason,
}

#[derive(Serialize)]
struct Toolchains {
    installed: Vec<InstalledToolchain>,
    /// `None` if no default toolchain configured
    default: Option<DefaultToolchain>,
    /// `None` if no override for current directory configured, in which case `default` if any is used
    active_override: Option<Override>,
    /// Toolchain, if any, ultimately chosen based on `default` and `active_override`
    resolved_active: Option<Result<String>>,
}

#[derive(Serialize)]
pub struct StateDump {
    elan_version: Version,
    toolchains: Toolchains,
}

impl StateDump {
    pub fn new(cfg: &Cfg, no_net: bool) -> crate::Result<StateDump> {
        let newest = fetch_latest_release_tag("leanprover/elan", None, no_net);
        let ref cwd = utils::current_dir()?;
        let active_override = cfg.find_override(cwd)?;
        let default = match cfg.get_default()? {
            None => None,
            Some(d) => Some(lookup_unresolved_toolchain_desc(cfg, &d)?)
        };
        Ok(StateDump {
            elan_version: Version {
                current: env!("CARGO_PKG_VERSION").to_string(),
                newest: newest.map(|s| s.trim_start_matches('v').to_string()).map_err(|e| e.to_string()),
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
                    resolved: resolve_toolchain_desc(cfg, &default, no_net)
                      .map(|t| t.to_string())
                      .map_err(|e| e.to_string()),
                }),
                active_override: active_override.as_ref().map(|(desc, reason)| Override {
                    unresolved: desc.clone(),
                    reason: reason.clone(),
                }),
                resolved_active: active_override
                    .map(|p| p.0)
                    .or(default)
                    .map(|t| resolve_toolchain_desc(cfg, &t, no_net)
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
