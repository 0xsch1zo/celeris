use crate::utils;
use clap::builder::OsStr;
use itertools::Itertools;
use sanitize_filename;
use std::{
    fmt::Display,
    ops::ControlFlow,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub enum Error {
    AlreadyExists(String),
    NotFound(String),
    InvalidFilename(Box<dyn std::error::Error + Sync + Send>),
    InvalidLayoutName(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::NotFound(layout) => {
                format!("layout not found: {layout}")
            }
            Self::AlreadyExists(layout) => {
                format!("layout already exists: {layout}")
            }
            Self::InvalidFilename(_) => "failed to get a file name of path".to_owned(),
            Self::InvalidLayoutName(comment) => format!("invalid layout name: {comment}"),
        };
        write!(f, "{message}")
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidFilename(e) => Some(&**e),
            _ => None,
        }
    }
}

pub struct LayoutName(String);

impl LayoutName {
    const STORAGE_NAME_DELIMETER: &str = "_";
    const TMUX_NAME_DELIMETER: &str = "/";

    pub fn try_new(name: String) -> Result<Self, Error> {
        let tmux_special_chars = ['@', '$', '%', ':', '.'];
        if name.chars().any(|c| tmux_special_chars.contains(&c)) {
            return Err(Error::InvalidLayoutName(
                "name contains characters that tmux treats specially".to_owned(),
            ));
        }

        let sanitized = sanitize_filename::sanitize(name);
        Ok(Self(sanitized))
    }

    pub fn try_from_path(path: &Path, layout_manager: &LayoutManager) -> Result<Self, Error> {
        let mut name = utils::file_name(path).map_err(|e| Error::InvalidFilename(e.into()))?;
        let ancestors = path.ancestors().collect::<Vec<_>>();
        let ancestors: Vec<_> = ancestors
            .iter()
            .skip(1) // skip the original directory
            .enumerate()
            .take_while(|(i, _)| *i < ancestors.len() - 2) // acount for skip
            .map(|(_, a)| a)
            .map(|a| utils::file_name(a).map_err(|e| Error::InvalidFilename(e.into())))
            .collect::<Result<Vec<_>, _>>()?;

        let _ = ancestors.into_iter().try_for_each(|a| {
            if layout_manager.contains(&name) {
                name = format!("{}/{}", a, name);
                ControlFlow::Continue(())
            } else {
                ControlFlow::Break(())
            }
        });

        if layout_manager.contains(&name) {
            return Err(Error::AlreadyExists(name));
        }
        Self::try_new(name)
    }

    pub fn try_from_storage_name(storage_name: String) -> Result<Self, Error> {
        let name = storage_name.replace(Self::STORAGE_NAME_DELIMETER, Self::TMUX_NAME_DELIMETER);
        Self::try_new(name)
    }

    fn name(&self) -> &str {
        &self.0
    }

    fn storage_name(&self) -> String {
        self.0
            .replace(Self::TMUX_NAME_DELIMETER, Self::STORAGE_NAME_DELIMETER)
    }
}

#[derive(Debug)]
pub struct Layout {
    tmux_name: String,
    storage_name: String,
}

impl PartialEq for Layout {
    fn eq(&self, other: &Self) -> bool {
        self.tmux_name == other.tmux_name || self.storage_name == other.storage_name
    }
}

impl Layout {
    pub fn new(layout_name: LayoutName) -> Self {
        Self {
            tmux_name: layout_name.name().to_owned(),
            storage_name: layout_name.storage_name(),
        }
    }

    pub fn tmux_name(&self) -> &str {
        &self.tmux_name
    }

    pub fn storage_name(&self) -> &str {
        &self.storage_name
    }

    pub fn storage_path(&self, layouts_dir: &Path) -> PathBuf {
        layouts_dir
            .join(&self.storage_name)
            .with_extension(Self::extension())
    }

    pub fn extension() -> OsStr {
        "rhai".into()
    }
}

#[derive(Debug)]
pub struct LayoutManager {
    layouts: Vec<Layout>,
}

impl LayoutManager {
    pub fn new(layouts: Vec<Layout>) -> Self {
        Self { layouts }
    }

    pub fn layout(&self, name: &str) -> Option<&Layout> {
        self.layouts.iter().find(|entry| entry.tmux_name == name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.layouts.iter().find(|s| s.tmux_name == name).is_some()
    }

    pub fn list(&self) -> Vec<&String> {
        self.layouts.iter().map(|e| &e.tmux_name).collect_vec()
    }

    // impure
    pub fn create(&mut self, layout: Layout) -> Result<(), Error> {
        if self.layouts.contains(&layout) {
            return Err(Error::AlreadyExists(layout.tmux_name));
        }
        self.layouts.push(layout);
        Ok(())
    }

    // impure
    pub fn remove(&mut self, name: &str) -> Result<(), Error> {
        let exists = self.layouts.iter().any(|e| e.tmux_name == name);
        if !exists {
            return Err(Error::NotFound(name.to_owned()));
        }
        self.layouts.retain(|l| l.tmux_name != name);
        Ok(())
    }
}
