mod core;

use clap::builder::OsStr;
use delegate::delegate;
use itertools::Itertools;
use ref_cast::RefCast;
use std::fmt::Display;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::{error, fs};
use walkdir::WalkDir;

#[derive(Debug)]
pub enum Error {
    CoreError(Box<dyn error::Error + Send + Sync + 'static>),
    FSOperationFaiure(String, io::Error), // break down to pieces
    InvalidDirEntry(Box<dyn error::Error + Send + Sync + 'static>),
    InvalidFilename,
    NotFound(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::CoreError(_) => "error in manifest core".to_owned(),
            Self::FSOperationFaiure(desc, _) => {
                format!("manifest file operation failed: {desc}")
            }
            Self::InvalidDirEntry(_) => "invalid dir entry".to_owned(),
            Self::InvalidFilename => "filename contains invalid utf-8".to_owned(),
            Self::NotFound(layout) => format!("layout not found: {layout}"),
        };
        write!(f, "{message}")
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::FSOperationFaiure(_, e) => Some(e),
            Self::CoreError(e) => Some(&**e),
            Self::InvalidDirEntry(e) => Some(&**e),
            _ => None,
        }
    }
}

impl From<core::Error> for Error {
    fn from(value: core::Error) -> Self {
        Error::CoreError(Box::new(value))
    }
}

pub struct LayoutName {
    core: core::LayoutName,
}

impl LayoutName {
    fn try_new(name: String) -> Result<Self, Error> {
        let core = core::LayoutName::try_new(name)?;
        Ok(Self { core })
    }

    fn try_from_path(path: &Path, layout_manager: &LayoutManager) -> Result<Self, Error> {
        let core = core::LayoutName::try_from_path(path, &layout_manager.core)?;
        Ok(Self { core })
    }

    pub fn try_from_storage_name(storage_name: String) -> Result<Self, Error> {
        let core = core::LayoutName::try_from_storage_name(storage_name)?;
        Ok(Self { core })
    }
}

#[derive(Debug, RefCast)]
#[repr(transparent)]
pub struct Layout {
    core: core::Layout,
}

impl Layout {
    delegate! {
        to self.core {
            pub fn tmux_name(&self) -> &str;
            pub fn storage_name(&self) -> &str;
            pub fn storage_path(&self, layouts_path: &Path) -> PathBuf;
        }

        to core::Layout {
            pub fn extension() -> OsStr;
        }
    }
}

impl PartialEq for Layout {
    fn eq(&self, other: &Self) -> bool {
        self.core == other.core
    }
}

impl Layout {
    pub fn new(name: LayoutName) -> Self {
        Self {
            core: core::Layout::new(name.core),
        }
    }
}

pub struct LayoutManager {
    core: core::LayoutManager,
    layouts_dir: PathBuf,
}

impl LayoutManager {
    pub fn enumerate_layouts(layouts_dir: &Path) -> Result<Vec<core::Layout>, Error> {
        let paths: Vec<PathBuf> = WalkDir::new(layouts_dir)
            .into_iter()
            .map(|entry| -> Result<_, Error> {
                Ok(entry
                    .map_err(|e| Error::InvalidDirEntry(e.into()))?
                    .into_path())
            })
            .try_collect()?;

        Ok(paths
            .into_iter()
            .filter(|path| path.is_file())
            .filter(|path| path.extension() == Some(&OsStr::from("rhai")))
            .map(|path| {
                Ok(path
                    .file_stem()
                    .unwrap()
                    .to_str()
                    .ok_or(Error::InvalidFilename)?
                    .to_owned())
            })
            .map(|filename: Result<_, Error>| Ok(LayoutName::try_from_storage_name(filename?)?))
            .map(
                |layout_name: Result<_, Error>| -> Result<core::Layout, Error> {
                    Ok(core::Layout::new(layout_name?.core))
                },
            )
            .try_collect()?)
    }

    pub fn new(layouts_dir: PathBuf) -> Result<Self, Error> {
        let layouts = Self::enumerate_layouts(&layouts_dir)?;
        layouts.iter().for_each(|l| println!("{}", l.tmux_name()));
        let core = core::LayoutManager::new(layouts);
        Ok(Self { core, layouts_dir })
    }

    // delegate the pure ones that don't ned conversion
    delegate! {
        to self.core {
            pub fn contains(&self, name: &str) -> bool;
            pub fn list(&self) -> Vec<&String>;
        }
    }

    pub fn create(&mut self, layout: Layout) -> Result<(), Error> {
        File::create_new(&layout.storage_path(&self.layouts_dir)).map_err(|e| {
            Error::FSOperationFaiure(
                format!("failed to create layout with name: {}", layout.tmux_name()),
                e,
            )
        })?;
        self.core.create(layout.core)?;
        Ok(())
    }

    pub fn layout(&self, name: &str) -> Option<&Layout> {
        self.core.layout(name).map(Layout::ref_cast)
    }

    pub fn remove(self, name: &str) -> Result<(), Error> {
        let layout = self.layout(name).ok_or(Error::NotFound(name.to_owned()))?;
        fs::remove_file(layout.storage_path(&self.layouts_dir)).map_err(|e| {
            Error::FSOperationFaiure(
                format!(
                    "failed to remove layout file with name: {}",
                    layout.tmux_name()
                ),
                e,
            )
        })?;
        Ok(())
    }
}
