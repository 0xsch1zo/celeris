mod core;

use crate::directory_manager::{self, DirectoryManager};
use delegate::delegate;
use ref_cast::RefCast;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_with::{DeserializeAs, SerializeAs, serde_as};
use std::error;
use std::fmt::Display;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[derive(Debug)]
pub enum Error {
    CoreError(Box<dyn error::Error + Send + Sync + 'static>),
    CoreDirectoryErr(directory_manager::Error),
    FSOperationFaiure(String, io::Error), // break down to pieces
    SerializeFailure(toml::ser::Error),
    DeserializeFailure(toml::de::Error),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::CoreError(_) => "error in manifest core".to_owned(),
            Self::CoreDirectoryErr(_) => {
                "an error occured while operating on a directory core to the manifest".to_owned()
            }
            Self::FSOperationFaiure(desc, _) => {
                format!("manifest file operation failed: {desc}")
            }
            Self::SerializeFailure(_) => "failed to serialize the manifest".to_owned(),
            Self::DeserializeFailure(_) => "failed to deserialize the manifest".to_owned(),
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
            Self::CoreError(e) => Some(&**e),
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

impl From<directory_manager::Error> for Error {
    fn from(value: directory_manager::Error) -> Self {
        Error::CoreDirectoryErr(value)
    }
}

impl From<core::Error> for Error {
    fn from(value: core::Error) -> Self {
        Error::CoreError(Box::new(value))
    }
}

#[derive(Debug, RefCast)]
#[repr(transparent)]
pub struct Entry {
    core: core::Entry,
}

impl Entry {
    delegate! {
        to self.core {
            pub fn name(&self) -> &str;
            pub fn session_path(&self) -> &Path;
            pub fn script_name(&self) -> &str;
        }
    }
}

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.core == other.core
    }
}

impl Entry {
    pub fn new(name: String, session_path: PathBuf) -> Self {
        Self {
            core: core::Entry::new(name, session_path),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(remote = "core::Entry")]
struct EntryCoreDef {
    name: String,
    session_path: PathBuf,
    script_name: String,
}

impl SerializeAs<core::Entry> for EntryCoreDef {
    fn serialize_as<S>(value: &core::Entry, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        EntryCoreDef::serialize(value, serializer)
    }
}

impl<'de> DeserializeAs<'de, core::Entry> for EntryCoreDef {
    fn deserialize_as<D>(deserializer: D) -> Result<core::Entry, D::Error>
    where
        D: Deserializer<'de>,
    {
        EntryCoreDef::deserialize(deserializer)
    }
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
#[serde(remote = "core::Manifest")]
struct ManifestCoreDef {
    #[serde_as(as = "Vec<EntryCoreDef>")]
    entries: Vec<core::Entry>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(transparent)]
struct ManifestCore(#[serde(with = "ManifestCoreDef")] core::Manifest);

impl ManifestCore {
    delegate! {
        to self.0 {
            fn deduce_name(&self, path: &Path) -> Result<String, core::Error>;
            fn entry(&self, name: &str) -> Option<&core::Entry>;
            fn contains(&self, name: &str) -> bool;
            fn list(&self) -> Vec<&String>;
            fn extend(self, entry: core::Entry) -> Result<core::Manifest, core::Error>;
            fn filter_out(self, name: &str) -> Result<core::Manifest, core::Error>;
        }
    }
}

pub struct Manifest {
    core: ManifestCore,
    dir_mgr: Rc<DirectoryManager>,
}

impl Manifest {
    fn path(dir_mgr: &DirectoryManager) -> Result<PathBuf, Error> {
        const MANIFEST_FILE: &'static str = ".manifest.toml";
        Ok(dir_mgr.config_dir()?.join(MANIFEST_FILE))
    }

    fn serialize(&self) -> Result<(), Error> {
        let path = Self::path(&self.dir_mgr)?;
        fs::write(&path, toml::to_string(&self.core)?).map_err(|e| {
            Error::FSOperationFaiure("failed to write to manifest file".to_owned(), e)
        })?;
        Ok(())
    }

    fn deserialize(dir_mgr: &DirectoryManager) -> Result<ManifestCore, Error> {
        let path = Self::path(dir_mgr)?;
        let manifest_str = fs::read_to_string(path)
            .map_err(|e| Error::FSOperationFaiure("couldn't read manifest file".to_owned(), e))?;

        Ok(toml::from_str(&manifest_str)?)
    }

    pub fn new(dir_mgr: Rc<DirectoryManager>) -> Result<Self, Error> {
        let path = Self::path(&dir_mgr)?;
        let core = match path.exists() {
            true => Self::deserialize(&dir_mgr)?,
            false => ManifestCore::default(),
        };
        Ok(Self { core, dir_mgr })
    }

    // delegate the pure ones that don't ned conversion
    delegate! {
        to self.core {
            pub fn contains(&self, name: &str) -> bool;
            pub fn list(&self) -> Vec<&String>;
        }
    }

    pub fn deduce_name(&self, path: &Path) -> Result<String, Error> {
        let name = self.core.deduce_name(path)?;
        Ok(name)
    }

    pub fn extend(self, entry: Entry) -> Result<Manifest, Error> {
        let manifest = Manifest {
            core: ManifestCore(self.core.extend(entry.core)?),
            ..self
        };
        manifest.serialize()?;
        Ok(manifest)
    }

    pub fn entry(&self, name: &str) -> Option<&Entry> {
        self.core.entry(name).map(Entry::ref_cast)
    }

    pub fn filter_out(self, name: &str) -> Result<Self, Error> {
        let manifest = Manifest {
            core: ManifestCore(self.core.filter_out(name)?),
            ..self
        };
        manifest.serialize()?;
        Ok(manifest)
    }
}
