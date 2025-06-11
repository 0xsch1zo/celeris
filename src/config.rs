use serde::Deserialize;
use std::env;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::PathBuf;

const CONFIG_DIR: &'static str = "sesh";
const CONFIG_FILE: &'static str = "config.toml";

#[derive(Deserialize, Debug)]
pub struct Config {
    search_roots: Vec<SearchRoot>,
}

#[derive(Deserialize, Debug)]
pub struct SearchRoot {
    path: String,
    depth: Option<u32>,
    exclude: Option<String>,
}

pub enum Error {
    NotFound,
    ParsingError(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParsingError(msg) => write!(f, "Parsing error: {msg}"),
            Self::NotFound => write!(
                f,
                "Couldn't find the main sesh config at \n\
                - $XDG_CONFIG_HOME/{CONFIG_DIR}/{CONFIG_FILE} or \n\
                - $HOME/.config/{CONFIG_DIR}/{CONFIG_FILE} or \n\
                - $USER/home/.config/{CONFIG_DIR}/{CONFIG_FILE}"
            ),
        }
    }
}

impl Config {
    pub fn new() -> Result<Self, Error> {
        let config_path = Self::find_config_path()?;
        let config = fs::read_to_string(&config_path).map_err(|_| Error::NotFound)?;

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
            .map_err(|_: env::VarError| Error::NotFound)?;

        Ok(config_path.canonicalize().map_err(|_| Error::NotFound)?)
    }
}
