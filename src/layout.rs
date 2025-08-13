mod core;

use core::ExtractLayoutsIterator;
use delegate::delegate;
use handlebars::{Handlebars, RenderError};
use itertools::Itertools;
use ref_cast::RefCast;
use serde::Serialize;
use std::env::VarError;
use std::ffi::OsString;
use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, io};
use std::{error, fs};
use walkdir::WalkDir;

use crate::config::Config;

#[derive(Debug)]
pub enum Error {
    CoreError(Box<dyn error::Error + Send + Sync + 'static>),
    FSOperationFaiure(String, io::Error), // break down to pieces
    FailedCommand(String, io::Error),
    InvalidDirEntry(Box<dyn error::Error + Send + Sync + 'static>),
    NotFound(String),
    EditorNotFound,
    EditorInvalid(OsString),
    TemplateRenderError(String, RenderError),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::CoreError(_) => "error in layout manager core".to_owned(),
            Self::FSOperationFaiure(desc, _) => {
                format!("layout manager file operation failed: {desc}")
            }
            Self::InvalidDirEntry(_) => "invalid dir entry".to_owned(),
            Self::NotFound(layout) => format!("layout not found: {layout}"),
            Self::FailedCommand(command, _) => format!("failed to execute command: {command}"),
            Self::EditorNotFound => "$EDITOR is not set nor set in the config".to_owned(),
            Self::EditorInvalid(invalid_text) => {
                format!("$EDITOR contains invalid unicode: {invalid_text:?}")
            }
            Self::TemplateRenderError(comment, _) => {
                format!("Failed to render layout template: {comment}")
            }
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
            Self::FailedCommand(_, e) => Some(e),
            Self::TemplateRenderError(_, e) => Some(e),
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
    pub fn try_new(tmux_name: String) -> Result<Self, Error> {
        let core = core::LayoutName::try_new(tmux_name)?;
        Ok(Self { core })
    }

    pub fn try_from_path(path: &Path, layout_manager: &LayoutManager) -> Result<Self, Error> {
        let core = core::LayoutName::try_from_path(path, &layout_manager.core)?;
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
            pub fn storage_path(&self, layouts_path: &Path) -> PathBuf;
        }
    }
}

impl PartialEq for Layout {
    fn eq(&self, other: &Self) -> bool {
        self.core == other.core
    }
}

impl Layout {
    pub fn new(tmux_name: LayoutName) -> Self {
        Self {
            core: core::Layout::new(tmux_name.core),
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
            .map(|path| {
                let is_file = path.is_file();
                core::LayoutInfo::new(path, is_file)
            })
            .extract_layouts()
            .try_collect()?)
    }

    pub fn new(layouts_dir: PathBuf) -> Result<Self, Error> {
        let layouts = Self::enumerate_layouts(&layouts_dir)?;
        let core = core::LayoutManager::new(layouts);
        Ok(Self { core, layouts_dir })
    }

    // delegate the pure ones that don't ned conversion
    delegate! {
        to self.core {
            pub fn list(&self) -> Vec<&String>;
        }
    }

    pub fn create(&mut self, layout: Layout, root: &Path) -> Result<(), Error> {
        let template = template(TemplateData::new(layout.tmux_name(), &root))?;
        fs::write(&layout.storage_path(&self.layouts_dir), template).map_err(|e| {
            Error::FSOperationFaiure(
                format!(
                    "failed to create layout with tmux_name: {}",
                    layout.tmux_name()
                ),
                e,
            )
        })?;
        self.core.create(layout.core)?;
        Ok(())
    }

    pub fn layout(&self, tmux_name: &str) -> Option<&Layout> {
        self.core.layout(tmux_name).map(Layout::ref_cast)
    }

    pub fn remove(&mut self, tmux_name: &str) -> Result<(), Error> {
        self.core.remove(tmux_name)?;
        let layout = self
            .layout(tmux_name)
            .ok_or(Error::NotFound(tmux_name.to_owned()))?;
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

    pub fn edit(&self, tmux_name: &str, config: &Config) -> Result<(), Error> {
        let editor = config
            .editor
            .clone()
            .unwrap_or(env::var("EDITOR").map_err(|e| match e {
                VarError::NotPresent => Error::EditorNotFound,
                VarError::NotUnicode(invalid_text) => Error::EditorInvalid(invalid_text),
            })?);

        let layout = self
            .layout(tmux_name)
            .ok_or(Error::NotFound(tmux_name.to_owned()))?;
        let layout_path = layout.storage_path(&self.layouts_dir);
        Command::new(&editor)
            .arg(layout_path)
            .status()
            .map_err(|e| Error::FailedCommand(editor, e))?;
        Ok(())
    }
}

#[derive(Serialize)]
pub struct TemplateData<'a> {
    session_root: &'a Path,
    session_name: &'a str,
}

impl<'a> TemplateData<'a> {
    pub fn new(session_name: &'a str, session_root: &'a Path) -> Self {
        Self {
            session_root,
            session_name,
        }
    }
}

fn template(data: TemplateData) -> Result<String, Error> {
    let handlbars = Handlebars::new();
    let template = include_str!("../templates/default.lua");
    Ok(handlbars.render_template(template, &data).map_err(|e| {
        Error::TemplateRenderError("failed to render default layout template".to_owned(), e)
    })?)
}
