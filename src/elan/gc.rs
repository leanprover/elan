use std::{collections::HashSet, path::{Path, PathBuf}};

use itertools::Itertools;

use crate::{Cfg, Toolchain};

fn get_root_file(cfg: &Cfg) -> PathBuf {
    cfg.elan_dir.join("known-projects")
}

fn get_roots(cfg: &Cfg) -> elan_utils::Result<Vec<String>> {
    let path = get_root_file(cfg);
    if path.exists() {
        let roots = std::fs::read_to_string(&path)?;
        Ok(roots.split("\n").map(|s| s.to_string()).collect_vec())
    } else {
        Ok(vec![])
    }
}

pub fn add_root(cfg: &Cfg, root: &Path) -> elan_utils::Result<()> {
    let path = get_root_file(cfg);
    let mut roots = get_roots(cfg)?;
    let root = root.to_str().unwrap().to_string();
    if !roots.contains(&root) {
        roots.push(root);
        let roots = roots.join("\n");
        std::fs::write(path, roots)?;
    }
    Ok(())
}

pub fn get_unreachable_toolchains(cfg: &Cfg) -> crate::Result<Vec<Toolchain>> {
    let roots = get_roots(cfg)?;
    let roots = roots.into_iter().filter_map(|r| {
        let path = PathBuf::from(r).join("lean-toolchain");
        if path.exists() {
            Some(std::fs::read_to_string(path).unwrap().trim().to_string())
        } else {
            None
        }
    }).collect::<HashSet<_>>();
    Ok(cfg.list_toolchains()?.into_iter()
        .map(|t| Toolchain::from(cfg, &t))
        .filter(|t| !t.is_custom() && !roots.contains(&t.desc.to_string()))
        .collect_vec())
}
