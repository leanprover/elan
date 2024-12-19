use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use elan_dist::dist::ToolchainDesc;
use itertools::Itertools;

use crate::{
    lookup_unresolved_toolchain_desc, read_toolchain_desc_from_file, resolve_toolchain_desc_ext,
    Cfg, Toolchain,
};

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

pub fn analyze_toolchains(
    cfg: &Cfg,
) -> crate::Result<(Vec<Toolchain<'_>>, Vec<(String, ToolchainDesc)>)> {
    let roots = get_roots(cfg)?;
    let mut used_toolchains = roots
        .into_iter()
        .filter_map(|r| {
            let path = PathBuf::from(r.clone()).join("lean-toolchain");
            if let Ok(desc) = read_toolchain_desc_from_file(cfg, &path) {
                Some((r, desc))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    if let Some(default) = cfg.get_default()? {
        if let Ok(default) = resolve_toolchain_desc_ext(
            cfg,
            &lookup_unresolved_toolchain_desc(cfg, &default)?,
            true,
            true,
        ) {
            used_toolchains.push(("default toolchain".to_string(), default));
        }
    }
    if let Some(ref env_override) = cfg.env_override {
        if let Ok(desc) = resolve_toolchain_desc_ext(
            cfg,
            &lookup_unresolved_toolchain_desc(cfg, env_override)?,
            true,
            true,
        ) {
            used_toolchains.push(("ELAN_TOOLCHAIN".to_string(), desc));
        }
    }
    for (path, tc) in cfg.get_overrides()? {
        used_toolchains.push((format!("{} (override)", path), tc));
    }
    let used_toolchains_set = used_toolchains
        .iter()
        .map(|p| p.1.to_string())
        .collect::<HashSet<_>>();
    let unused_toolchains = cfg
        .list_toolchains()?
        .into_iter()
        .map(|t| Toolchain::from(cfg, &t))
        .filter(|t| !t.is_custom() && !used_toolchains_set.contains(&t.desc.to_string()))
        .collect_vec();
    Ok((unused_toolchains, used_toolchains))
}
