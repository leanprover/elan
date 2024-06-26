use Cfg;
use std::path::PathBuf;
use std::io::Result;

use serde_derive::Serialize;

use crate::OverrideReason;

#[derive(Serialize)]
struct Version {
    current: String,
    /// `None` on network error
    newest: Option<String>,
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
    /// Fully resolved name; `None` if `unresolved` needed to be resolved but there was a network error
    resolved: Option<String>,
}

#[derive(Serialize)]
struct Toolchains {
    installed: Vec<InstalledToolchain>,
    /// `None` if no default toolchain configured
    default: Option<DefaultToolchain>,
    /// `None` if no override for current directory configured, in which case `default` if any is used
    active_override: Option<OverrideReason>,
}

#[derive(Serialize)]
pub struct StateDump {
    elan_version: Version,
    toolchains: Toolchains,
}

impl StateDump {
    pub fn new(_cfg: &Cfg) -> Result<StateDump> {
        Ok(StateDump {
            elan_version: Version {
                current: unimplemented!(),
                newest: unimplemented!(),
            },
            toolchains: Toolchains {
                installed: unimplemented!(),
                default: unimplemented!(),
                active_override: unimplemented!(),
            },
        })
    }

    pub fn print(&self) -> Result<()> {
        unimplemented!()
    }
}
