use std::error;
use std::fmt::Display;
use std::fs;
use std::io;
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

const PROJECT_DIR_NAME: &'static str = "sesh";

pub fn config_dir() -> Result<PathBuf, Error> {
    let path: PathBuf = dirs::config_dir()
        // TODO: add flag do specify alt config location, include that in the error message
        .ok_or(Error::NotFound("config directory".to_owned()))?
        .join(PROJECT_DIR_NAME);

    if !path.exists() {
        return Err(Error::NotFound("config directory".to_owned()));
    }
    Ok(path)
}

pub fn internals_dir() -> Result<PathBuf, Error> {
    const INTERNALS_DIR: &'static str = "internals";
    let path = config_dir()?.join(INTERNALS_DIR);
    if !path.exists() {
        fs::create_dir(&path).map_err(|e| {
            Error::FSOperationFailed("failed to create internals directory".to_owned(), e)
        })?
    }
    Ok(path)
}

pub fn scripts_path() -> Result<PathBuf, Error> {
    const SCRIPTS_DIR: &'static str = "scripts";
    let scripts_path = config_dir()?.join(SCRIPTS_DIR);
    if !scripts_path.exists() {
        fs::create_dir(&scripts_path)
            .map_err(|e| Error::FSOperationFailed("failed to create scripts dir".to_owned(), e))?
    }
    Ok(scripts_path)
}
