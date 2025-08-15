use crate::directory_manager::DirectoryManager;
use color_eyre::eyre::Context;
use color_eyre::{Result, eyre};
use eyre::eyre;
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Config {
    pub editor: Option<String>,
    pub depth: usize,
    pub search_subdirs: bool,
    pub search_roots: Vec<SearchRoot>,
    pub excludes: Vec<String>,
    pub disable_template: bool,
    pub disable_editor_on_creation: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            editor: None,
            depth: 10,
            search_subdirs: false,
            search_roots: Vec::new(),
            excludes: Vec::new(),
            disable_template: false,
            disable_editor_on_creation: false,
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct SearchRoot {
    pub path: String,
    pub depth: Option<usize>,
    pub excludes: Option<Vec<String>>,
}

pub enum PathType {
    SearchRoot,
    ExcludeDirectory,
}

impl Config {
    pub fn new(dir_mgr: &DirectoryManager) -> Result<Self> {
        const CONFIG_FILE: &'static str = "config.toml";
        let config_path = dir_mgr.config_dir()?.join(CONFIG_FILE);
        let config = fs::read_to_string(&config_path).wrap_err(format!(
            "main sesh config not found in path: {config_path:?}"
        ))?;
        let config: Config = toml::from_str(&config).wrap_err("parsing error")?;
        Self::validate_config(&config)?;
        Ok(config)
    }

    fn validate_config(&self) -> Result<()> {
        self.search_roots
            .iter()
            .map(|root| {
                let root_path = Path::new(&root.path);
                if !root_path.exists() {
                    return Err(eyre!("path not found: {}", root.path.clone()));
                } else if !root_path.is_dir() {
                    return Err(eyre!("path is not a directory: {}", root.path.clone()));
                }

                Ok(())
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(())
    }
}
