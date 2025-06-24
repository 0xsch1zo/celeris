use crate::repos::Repo;
use crate::utils;
use color_eyre::eyre::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
pub struct Manifest {
    #[serde(default = "default_entries")]
    entries: Vec<Entry>,
}

#[derive(Serialize, Deserialize)]
struct Entry {
    name: String,
    path: PathBuf,
    hash: String,
}

fn default_entries() -> Vec<Entry> {
    Vec::new()
}

impl Manifest {
    pub fn new() -> Result<Self> {
        let path = utils::config_dir()?.join(Self::MANIFEST_FILE);
        if path.exists() {
            let manifest = fs::read_to_string(path).wrap_err("Couldn't read manifest file")?;
            Ok(toml::from_str(&manifest).wrap_err("Failed to deserialize manifest")?)
        } else {
            Ok(Manifest {
                entries: Vec::new(),
            })
        }
    }

    const MANIFEST_FILE: &'static str = "manifest.toml";
    pub fn serialize(&self) -> Result<()> {
        let manifest = toml::to_string(&self).wrap_err("Failed to serialize manifest")?;
        let path = utils::config_dir()?.join(Self::MANIFEST_FILE);
        fs::write(&path, &manifest)
            .wrap_err_with(|| format!("Failed to write to manifest file at: {path:?}",))?;
        Ok(())
    }

    pub fn push_unique(&mut self, repo: Repo) -> Result<()> {
        let new_hash = format!(
            "{:x}",
            md5::compute(
                utils::path_to_string(repo.path.as_path()).wrap_err("Failed to hash path")?,
            ),
        );
        if self.entries.iter().any(|entry| entry.hash == new_hash) {
            return Ok(());
        }

        self.entries.push(Entry {
            hash: new_hash,
            name: repo.name,
            path: repo.path,
        });

        Self::serialize(self)?;
        Ok(())
    }

    /*pub fn update_diff(&mut self, repos: &[Repo]) -> Result<()> {
        Ok(self.update(self.diff(repos)?)?)
    }

    fn diff(&self, repos: &[Repo]) -> Result<Vec<Repo>> {
        let hashes = repos
            .iter()
            .map(|repo| -> Result<String> {
                Ok(format!(
                    "{:x}",
                    md5::compute(
                        utils::path_to_string(repo.path.as_path())
                            .wrap_err("Failed to hash path")?,
                    ),
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(repos
            .into_iter()
            .zip(hashes)
            .into_iter()
            .filter(|(_, hash)| !self.entries.iter().any(|entry| entry.hash == *hash))
            .map(|(repo, _)| repo.clone())
            .collect::<Vec<_>>())
    }

    fn update(&mut self, repos: Vec<Repo>) -> Result<()> {
        let mut entries = repos
            .into_iter()
            .map(|repo| -> Result<Entry> {
                Ok(Entry {
                    hash: format!(
                        "{:x}",
                        md5::compute(
                            utils::path_to_string(&repo.path.as_path())
                                .wrap_err("Failed to hash path")?
                        )
                    ),
                    name: repo.name,
                    path: repo.path,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(self.entries.append(&mut entries))
    }*/
}
