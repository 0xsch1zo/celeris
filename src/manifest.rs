use crate::pdirs;
use crate::utils;
use color_eyre::eyre::{self, Context};
use serde::{Deserialize, Serialize};
use std::error;
use std::fmt::Display;
use std::fs;
use std::io;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum Error {
    NotFound(String),
    AlreadyExists(String),
    CoreDirectoryErr(pdirs::Error),
    FSOperationFaiure(String, io::Error), // break down to pieces
    SerializeFailure(toml::ser::Error),
    DeserializeFailure(toml::de::Error),
    DeductionFilenameErr(Box<dyn error::Error + Send + Sync>),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::NotFound(entry) => format!("manifest entry not found: {entry}"),
            Self::AlreadyExists(entry) => format!("manifest entry already exists: {entry}"),
            Self::CoreDirectoryErr(_) => {
                "an error occured while operating on a directory core to the manifest".to_owned()
            }
            Self::FSOperationFaiure(desc, _) => {
                format!("manifest file operation failed: {desc}")
            }
            Self::SerializeFailure(_) => "failed to serialize the manifest".to_owned(),
            Self::DeserializeFailure(_) => "failed to deserialize the manifest".to_owned(),
            Self::DeductionFilenameErr(_) => {
                "failed to get a file name while deducing the name of a session".to_owned()
            }
        };
        write!(f, "{message}")
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::FSOperationFaiure(_, e) => Some(e),
            Self::SerializeFailure(e) => Some(e),
            Self::DeserializeFailure(e) => Some(e),
            Self::CoreDirectoryErr(e) => Some(e),
            Self::DeductionFilenameErr(e) => Some(&**e),
            _ => None,
        }
    }
}

impl From<toml::ser::Error> for Error {
    fn from(value: toml::ser::Error) -> Self {
        Error::SerializeFailure(value)
    }
}

impl From<toml::de::Error> for Error {
    fn from(value: toml::de::Error) -> Self {
        Error::DeserializeFailure(value)
    }
}

impl From<pdirs::Error> for Error {
    fn from(value: pdirs::Error) -> Self {
        Error::CoreDirectoryErr(value)
    }
}

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
        // TODO: handle same script_paths
        self.name == other.name
    }
}

// TODO: maybe use a map
impl Entry {
    pub fn new(name: String, session_path: PathBuf) -> eyre::Result<Self> {
        // TODO: use unique id instead of hash, or maybe not, idk think about it
        let hash = format!(
            "{:x}",
            md5::compute(
                utils::path_to_string(session_path.as_path()).wrap_err("failed to hash path")?,
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
    fn manifest_path() -> Result<PathBuf, Error> {
        const MANIFEST_FILE: &'static str = "manifest.toml";
        Ok(pdirs::internals_dir()?.join(MANIFEST_FILE))
    }

    fn scripts_path() -> Result<PathBuf, Error> {
        const SCRIPTS_DIR: &'static str = "scripts";
        let scripts_path = pdirs::internals_dir()?.join(SCRIPTS_DIR);
        if !scripts_path.exists() {
            fs::create_dir(&scripts_path).map_err(|e| {
                Error::FSOperationFaiure("failed to create scripts dir".to_owned(), e)
            })?
        }
        Ok(scripts_path)
    }

    pub fn new() -> Result<Self, Error> {
        let path = Self::manifest_path()?;
        if path.exists() {
            let manifest = fs::read_to_string(path).map_err(|e| {
                Error::FSOperationFaiure("couldn't read manifest file".to_owned(), e)
            })?;
            Ok(toml::from_str(&manifest)?)
        } else {
            Ok(Manifest {
                entries: Vec::new(),
            })
        }
    }

    fn serialize(&self) -> Result<(), Error> {
        let manifest = toml::to_string(&self)?;
        let path = Self::manifest_path()?;
        fs::write(&path, &manifest).map_err(|e| {
            Error::FSOperationFaiure("failed to write to manifest file".to_owned(), e)
        })?;
        Ok(())
    }

    pub fn deduce_name(&self, path: &Path) -> Result<String, Error> {
        if self.entries.iter().any(|e| e.session_path == path) {
            return Err(Error::AlreadyExists(path.to_string_lossy().to_string()));
        }

        let mut name = utils::file_name(path).map_err(|e| Error::DeductionFilenameErr(e.into()))?;
        let ancestors = path.ancestors().collect::<Vec<_>>();
        let ancestors = ancestors
            .iter()
            .skip(1)
            .enumerate()
            .take_while(|(i, _)| *i < ancestors.len() - 2) // acount for skip
            .map(|(_, a)| a)
            .map(|a| utils::file_name(a).map_err(|e| Error::DeductionFilenameErr(e.into())))
            .collect::<Result<Vec<_>, _>>()?;

        let _ = ancestors.into_iter().try_for_each(|a| {
            if self.contains(&name) {
                name = format!("{}/{}", a, name);
                ControlFlow::Continue(())
            } else {
                ControlFlow::Break(())
            }
        });

        if self.contains(&name) {
            return Err(Error::AlreadyExists(name));
        }

        Ok(name)
    }

    pub fn push(&mut self, entry: Entry) -> Result<(), Error> {
        if self.entries.contains(&entry) {
            return Err(Error::AlreadyExists(entry.name));
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

    pub fn remove(&mut self, name: &str) -> Result<(), Error> {
        self.entries.remove(
            self.entries
                .iter()
                .position(|e| e.name == name)
                .ok_or(Error::NotFound(name.to_owned()))?,
        );

        Self::serialize(self)?;
        Ok(())
    }
}
