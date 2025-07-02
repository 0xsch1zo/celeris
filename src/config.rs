use crate::utils;
use color_eyre::eyre::Context;
use color_eyre::{Result, eyre};
use eyre::eyre;
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub editor: Option<String>,
    pub filter_command: FilterCommand,
    #[serde(default = "default_depth")]
    pub depth: usize,
    #[serde(default = "default_search_subdirs")]
    pub search_subdirs: bool,
    pub search_roots: Vec<SearchRoot>,
    pub excludes: Option<Vec<String>>,
}

#[derive(Deserialize, Debug)]
pub struct FilterCommand {
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Deserialize, Debug)]
pub struct SearchRoot {
    pub path: String,
    pub depth: Option<usize>,
    pub excludes: Option<Vec<String>>,
}

fn default_search_subdirs() -> bool {
    false
}

fn default_depth() -> usize {
    10
}

pub enum PathType {
    SearchRoot,
    ExcludeDirectory,
}

impl Config {
    pub fn new() -> Result<Self> {
        const CONFIG_FILE: &'static str = "config.toml";
        let config_path = utils::config_dir()?.join(CONFIG_FILE);
        let config = fs::read_to_string(&config_path).wrap_err("main sesh config not found")?;

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
