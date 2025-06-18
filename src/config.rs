use color_eyre::eyre::Context;
use color_eyre::{Result, eyre};
use eyre::eyre;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

const CONFIG_DIR: &'static str = "sesh";
const CONFIG_FILE: &'static str = "config.toml";

#[derive(Deserialize, Debug)]
pub struct Config {
    pub search_roots: Vec<SearchRoot>,
    pub excludes: Option<Vec<String>>,
    #[serde(default = "default_depth")]
    pub depth: usize,
    #[serde(default = "default_search_subdirs")]
    pub search_subdirs: bool,
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
        let config_path = Self::find_config_path()?;
        let config = fs::read_to_string(&config_path).wrap_err("Main sesh config not found")?;

        let config: Config = toml::from_str(&config).wrap_err("Parsing error")?;

        Self::validate_config(&config)?;
        Ok(config)
    }

    fn validate_config(&self) -> Result<()> {
        self.search_roots
            .iter()
            .map(|root| {
                let root_path = Path::new(&root.path);
                if !root_path.exists() {
                    return Err(eyre!("Path not found: {}", root.path.clone()));
                } else if !root_path.is_dir() {
                    return Err(eyre!("Path is not a directory: {}", root.path.clone()));
                }

                Ok(())
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(())
    }

    fn find_config_path() -> Result<PathBuf> {
        let config_path: PathBuf = dirs::config_dir()
            .ok_or(eyre!("Couldn't find config dir to look for config"))?
            .join(CONFIG_DIR)
            .join(CONFIG_FILE);

        Ok(config_path
            .canonicalize()
            .wrap_err("Main sesh config not found")?)
    }
}
