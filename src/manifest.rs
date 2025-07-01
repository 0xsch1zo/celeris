use crate::internals_dir::internals_dir;
use crate::session_manager::SessionProperties;
use crate::utils;
use color_eyre::eyre::{Context, OptionExt, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
pub struct Manifest {
    #[serde(default = "default_entries")]
    entries: Vec<Entry>,
}

#[derive(Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub hash: String,
}

fn default_entries() -> Vec<Entry> {
    Vec::new()
}

impl Manifest {
    fn manifest_path() -> Result<PathBuf> {
        const MANIFEST_FILE: &'static str = "manifest.toml";
        Ok(internals_dir()?.join(MANIFEST_FILE))
    }

    pub fn new() -> Result<Self> {
        let path = Self::manifest_path()?;
        if path.exists() {
            let manifest = fs::read_to_string(path).wrap_err("Couldn't read manifest file")?;
            Ok(toml::from_str(&manifest).wrap_err("Failed to deserialize manifest")?)
        } else {
            Ok(Manifest {
                entries: Vec::new(),
            })
        }
    }

    fn serialize(&self) -> Result<()> {
        let manifest = toml::to_string(&self).wrap_err("Failed to serialize manifest")?;
        let path = Self::manifest_path()?;
        fs::write(&path, &manifest)
            .wrap_err_with(|| format!("Failed to write to manifest file at: {path:?}",))?;
        Ok(())
    }

    pub fn push_unique(&mut self, props: SessionProperties) -> Result<()> {
        let new_hash = format!(
            "{:x}",
            md5::compute(
                utils::path_to_string(props.path.as_path()).wrap_err("Failed to hash path")?,
            ),
        );
        if self.entries.iter().any(|entry| entry.hash == new_hash) {
            return Ok(());
        }

        self.entries.push(Entry {
            hash: new_hash,
            name: props.name,
            path: props.path,
        });

        Self::serialize(self)?;
        Ok(())
    }

    pub fn entry(&self, name: &str) -> Result<&Entry> {
        self.entries
            .iter()
            .find(|entry| entry.name == name)
            .ok_or_eyre(format!("manifest entry: {name}: not found"))
    }
}
