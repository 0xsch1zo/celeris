use serde::Deserialize;
use std::env;
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
    pub depth: Option<u32>,
    pub excludes: Option<Vec<String>>,
}

pub enum Error {
    ConfigNotFound,
    ParsingError(String),
    PathNotFound(String),
}

pub enum PathType {
    SearchRoot,
    ExcludeDirectory,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParsingError(msg) => write!(f, "Parsing error: {msg}"),
            Self::ConfigNotFound => write!(
                f,
                "Couldn't find the main sesh config at \n\
                - $XDG_CONFIG_HOME/{CONFIG_DIR}/{CONFIG_FILE} or \n\
                - $HOME/.config/{CONFIG_DIR}/{CONFIG_FILE} or \n\
                - $USER/home/.config/{CONFIG_DIR}/{CONFIG_FILE}"
            ),
            Self::PathNotFound(path) => write!(f, "Path not found: {path}"),
        }
    }
}

impl Config {
    pub fn new() -> Result<Self, Error> {
        let config_path = Self::find_config_path()?;
        let config = fs::read_to_string(&config_path).map_err(|_| Error::ConfigNotFound)?;

        let config: Config = toml::from_str(&config).map_err(|error| {
            Error::ParsingError(format!(
                "{}: {}",
                error
                    .span()
                    .and_then(|range| { Some(format!("{}:{}", range.start, range.end)) })
                    .unwrap_or("".to_string()),
                error.message()
            ))
        })?;

        println!("{config:?}");
        Ok(config)
    }

    fn validate_config(&self) -> Result<(), Error> {
        self.search_roots.iter().try_for_each(|root| {
            match Path::new(&root.path).exists() {
                false => return Err(Error::PathNotFound(root.path.clone())),
                _ => (),
            };

            match &root.excludes {
                None => return Ok(()),
                Some(excludes) => excludes
                    .iter()
                    .try_for_each(|exclude_path| match Path::new(&exclude_path).exists() {
                        false => return Err(Error::PathNotFound(root.path.clone())),
                        _ => Ok(()),
                    })?,
            };

            Ok(())
        })?;

        match self.exclude_directories {
            None => return Ok(()),
            Some(exclude_directories) => exclude_directories.
        }?;
    }

    fn find_config_path() -> Result<PathBuf, Error> {
        let config_path: PathBuf = env::var("XDG_CONFIG_HOME")
            .and_then(|config_home| {
                Ok(PathBuf::from(&config_home)
                    .join(CONFIG_DIR)
                    .join(CONFIG_FILE))
            })
            .or_else(|_| {
                let home = env::var("HOME")?;
                Ok(PathBuf::from(home)
                    .join(".config")
                    .join(CONFIG_DIR)
                    .join(CONFIG_FILE))
            })
            .or_else(|_: env::VarError| {
                let user = env::var("USER")?;
                Ok(PathBuf::from("/home")
                    .join(user)
                    .join(".config")
                    .join(CONFIG_DIR)
                    .join(CONFIG_FILE))
            })
            .map_err(|_: env::VarError| Error::ConfigNotFound)?;

        Ok(config_path
            .canonicalize()
            .map_err(|_| Error::ConfigNotFound)?)
    }
}
