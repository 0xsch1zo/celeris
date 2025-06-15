use serde::Deserialize;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

const CONFIG_DIR: &'static str = "sesh";
const CONFIG_FILE: &'static str = "config.toml";

#[derive(Deserialize, Debug)]
pub struct Config {
    pub search_roots: Vec<SearchRoot>,
    pub exclude_directories: Option<Vec<String>>,
}

#[derive(Deserialize, Debug)]
pub struct SearchRoot {
    pub path: String,
    #[serde(default = "default_depth")]
    pub depth: usize,
    pub excludes: Option<Vec<String>>,
}

fn default_depth() -> usize {
    10
}

pub enum Error {
    ConfigNotFound,
    ParsingError(String),
    NoSuchDirectory(String),
}

pub enum PathType {
    SearchRoot,
    ExcludeDirectory,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParsingError(msg) => write!(f, "Parsing error: {msg}"),
            Self::ConfigNotFound => write!(f, "Couldn't find the main sesh config"),
            Self::NoSuchDirectory(path) => write!(f, "No such directory: {path}"),
        }
    }
}

impl Config {
    pub fn new() -> Result<Self, Error> {
        let config_path = Self::find_config_path()?;
        let config = fs::read_to_string(&config_path).map_err(|_| Error::ConfigNotFound)?;

        let config: Config = toml::from_str(&config)
            .map_err(|error| Error::ParsingError(error.message().to_string()))?;

        println!("{config:?}");
        Self::validate_config(&config)?;
        Ok(config)
    }

    fn validate_config(&self) -> Result<(), Error> {
        self.search_roots
            .iter()
            .map(|root| {
                let root_path = Path::new(&root.path);
                if !root_path.is_dir() {
                    return Err(Error::NoSuchDirectory(root.path.clone()));
                }

                if let Some(excludes) = &root.excludes {
                    Self::validate_directories(excludes)?;
                }

                Ok(())
            })
            .collect::<Result<Vec<_>, _>>()?;

        // Global excludes
        if let Some(excludes) = &self.exclude_directories {
            Self::validate_directories(excludes)?;
        }

        Ok(())
    }

    fn validate_directories(paths: &Vec<String>) -> Result<(), Error> {
        paths
            .iter()
            .map(|path_str| {
                let path = Path::new(&path_str);
                match !path.is_absolute() || path.is_dir() {
                    true => Ok(()),
                    false => Err(Error::NoSuchDirectory(path_str.to_string())),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(())
    }

    fn find_config_path() -> Result<PathBuf, Error> {
        let config_path: PathBuf = dirs::config_dir()
            .ok_or(Error::ConfigNotFound)?
            .join(CONFIG_DIR)
            .join(CONFIG_FILE);

        Ok(config_path
            .canonicalize()
            .map_err(|_| Error::ConfigNotFound)?)
    }
}
