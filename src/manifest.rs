use crate::internals_dir::internals_dir;
use crate::session_manager::SessionProperties;
use crate::utils;
use color_eyre::eyre::{Context, OptionExt, Result, eyre};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize)]
pub struct Manifest {
    #[serde(default = "default_entries")]
    entries: Vec<Entry>,
}

#[derive(Serialize, Deserialize)]
pub struct Entry {
    name: String,
    session_path: PathBuf,
    script_path: PathBuf,
}

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Entry {
    pub fn new(name: String, session_path: PathBuf) -> Result<Self> {
        // TODO: use unique id instead of hash, or maybe not, idk think about it
        let hash = format!(
            "{:x}",
            md5::compute(
                utils::path_to_string(session_path.as_path()).wrap_err("Failed to hash path")?,
            )
        );
        let script_path = Manifest::scripts_path()?.join(hash).with_extension("rhai");
        Ok(Self {
            name,
            session_path,
            script_path,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn session_path(&self) -> &Path {
        self.session_path.as_path()
    }

    pub fn script_path(&self) -> &Path {
        self.script_path.as_path()
    }
}

fn default_entries() -> Vec<Entry> {
    Vec::new()
}

// TODO: handle the case when a new repo with the same name is added but with a different path
// This is possilbe because RepoManager disambiguates only the currenly found repos
impl Manifest {
    fn manifest_path() -> Result<PathBuf> {
        const MANIFEST_FILE: &'static str = "manifest.toml";
        Ok(internals_dir()?.join(MANIFEST_FILE))
    }

    fn scripts_path() -> Result<PathBuf> {
        const SCRIPTS_DIR: &'static str = "scripts";
        let scripts_path = internals_dir()?.join(SCRIPTS_DIR);
        if !scripts_path.exists() {
            fs::create_dir(&scripts_path)
                .wrap_err_with(|| format!("failed to create scripts dir at {scripts_path:?}"))?;
        }
        Ok(scripts_path)
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
        let entry = Entry::new(props.name, props.path).wrap_err("failed to create entry")?;
        if self.entries.contains(&entry) {
            return Err(eyre!("manifest entry already exists"));
        }

        self.entries.push(entry);
        Self::serialize(self)?;
        Ok(())
    }

    pub fn entry(&self, name: &str) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.name == name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.entries.iter().find(|s| s.name == name).is_some()
    }

    pub fn remove(&mut self, name: &str) -> Result<()> {
        self.entries.remove(
            self.entries
                .iter()
                .position(|e| e.name == name)
                .ok_or_eyre("manifest: entry not found")?,
        );

        Self::serialize(self)?;
        Ok(())
    }
}
