//! Lean distribution v2 manifests.
//!
//! This manifest describes the distributable artifacts for a single
//! release of Lean. They are toml files, typically downloaded from
//! e.g. static.lean-lang.org/dist/channel-lean-nightly.toml. They
//! describe where to download, for all platforms, each component of
//! the a release, and their relationships to each other.
//!
//! Installers use this info to customize Lean installations.
//!
//! See tests/channel-lean-nightly-example.toml for an example.

use crate::errors::*;
use elan_utils::toml_utils::*;
use toml;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Component {
    pub pkg: String,
}

impl Component {
    pub fn from_toml(mut table: toml::value::Table, path: &str) -> Result<Self> {
        Ok(Component {
            pkg: get_string(&mut table, "pkg", path)?,
        })
    }
    pub fn to_toml(self) -> toml::value::Table {
        let mut result = toml::value::Table::new();
        result.insert("pkg".to_owned(), toml::Value::String(self.pkg));
        result
    }
    pub fn name(&self) -> String {
        format!("{}", self.pkg)
    }
    pub fn description(&self) -> String {
        format!("'{}'", self.pkg)
    }
}
