use std::{collections::HashMap, error::Error, fs, path::PathBuf};

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Manifest {
    pub package: Package,
    pub dependencies: HashMap<String, String>
}

#[derive(Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub entry_point: String
}

pub struct ManifestSearchResult {
    pub location: PathBuf,
    pub manifest: Manifest
}

pub fn search_manifest(mut path: PathBuf) -> Result<ManifestSearchResult> {
    loop {
        let manifest_path = path.join("nlf.toml");
        if manifest_path.exists() {
            let content = fs::read_to_string(&manifest_path)?;
            let manifest: Manifest = toml::from_str(&content)?;
            return Ok(ManifestSearchResult { location: manifest_path, manifest })
        }

        if let Some(parent_path) = path.parent() {
            path = parent_path.to_path_buf();
            continue
        }

        break
    }

    Err(anyhow!("No manifest was found"))
}