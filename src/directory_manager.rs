use std::error;
use std::fmt::Display;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug)]
pub enum Error {
    NotFound(String),
    FSOperationFailed(String, io::Error),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::NotFound(location) => format!("location not found: {}", location),
            Self::FSOperationFailed(desc, ..) => {
                format!("{desc}: filesystem operation failed")
            }
        };
        write!(f, "{message}")
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::NotFound(_) => None,
            Self::FSOperationFailed(_, e) => Some(e),
        }
    }
}

const PROJECT_DIR_NAME: &'static str = "celeris";

#[derive(Debug)]
pub struct DirectoryManager {
    custom_config_path: Option<PathBuf>,
    custom_cache_path: Option<PathBuf>,
}

impl DirectoryManager {
    pub fn new() -> Self {
        Self {
            custom_config_path: None,
            custom_cache_path: None,
        }
    }
    fn check_path(path: &Path, directory_type: &str) -> Result<(), Error> {
        if !path.exists() {
            return Err(Error::NotFound(format!(
                "custom {directory_type} directory: {:?}",
                path
            )));
        }
        Ok(())
    }

    pub fn set_config_dir(&mut self, path: PathBuf) -> Result<&mut Self, Error> {
        Self::check_path(&path, "config")?;
        self.custom_config_path = Some(path);
        Ok(self)
    }

    pub fn set_cache_dir(&mut self, path: PathBuf) -> Result<&mut Self, Error> {
        Self::check_path(&path, "cache")?;
        self.custom_cache_path = Some(path);
        Ok(self)
    }

    pub fn config_dir(&self) -> Result<PathBuf, Error> {
        let default_path = dirs::config_dir();
        let default_path = default_path.as_ref();
        let path = match (self.custom_config_path.as_ref(), default_path) {
            (Some(path), _) => path.to_owned(),
            (None, Some(default_path)) => default_path.join(PROJECT_DIR_NAME),
            (None, None) => {
                return Err(Error::NotFound(
                    "local config directory, pass -c/--config to set custom config path".to_owned(),
                ));
            }
        };
        Ok(path)
    }

    pub fn layouts_dir(&self) -> Result<PathBuf, Error> {
        const LAYOUTS_DIR: &'static str = "layouts";
        let layouts_dir = self.config_dir()?.join(LAYOUTS_DIR);
        if !layouts_dir.exists() {
            fs::create_dir(&layouts_dir).map_err(|e| {
                Error::FSOperationFailed("failed to create scripts dir".to_owned(), e)
            })?
        }
        Ok(layouts_dir)
    }

    pub fn cache_dir(&self) -> Result<PathBuf, Error> {
        let default_path = dirs::cache_dir();
        let default_path = default_path.as_ref();
        let path = match (self.custom_cache_path.as_ref(), default_path) {
            (Some(path), _) => path.to_owned(),
            (None, Some(default_path)) => default_path.join(PROJECT_DIR_NAME),
            (None, None) => {
                return Err(Error::NotFound(
                    "local cache directory, pass -a/--cache-dir to set custom cache path"
                        .to_owned(),
                ));
            }
        };
        Ok(path)
    }
}
