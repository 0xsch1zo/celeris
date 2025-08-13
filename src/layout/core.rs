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

#[derive(Debug)]
pub struct LayoutName(String);

// FIXME: names with _ have a bug
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
        Ok(Self(name))
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

    fn tmux_name(&self) -> &str {
        &self.0
    }

    fn storage_name(&self) -> String {
        let storage_name = self
            .0
            .replace(Self::TMUX_NAME_DELIMETER, Self::STORAGE_NAME_DELIMETER);
        sanitize_filename::sanitize(storage_name)
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
            tmux_name: layout_name.tmux_name().to_owned(),
            storage_name: layout_name.storage_name(),
        }
    }

    pub fn tmux_name(&self) -> &str {
        &self.tmux_name
    }

    pub fn storage_path(&self, layouts_dir: &Path) -> PathBuf {
        layouts_dir
            .join(&self.storage_name)
            .with_extension(Self::extension())
    }

    pub fn extension() -> OsStr {
        "lua".into()
    }
}

pub struct LayoutInfo {
    path: PathBuf,
    is_file: bool,
}

impl LayoutInfo {
    pub fn new(path: PathBuf, is_file: bool) -> Self {
        Self { path, is_file }
    }
}

pub struct ExtractLayouts<'a> {
    iter: Box<dyn Iterator<Item = Result<Layout, Error>> + 'a>,
}

impl<'a> ExtractLayouts<'a> {
    pub fn new<I>(iter: I) -> Self
    where
        I: Iterator<Item = LayoutInfo> + 'a,
    {
        let iter = iter
            .filter(|info| info.is_file)
            .filter(|info| info.path.extension() == Some(&OsStr::from(Layout::extension())))
            .map(|info| {
                Ok(utils::file_stem(&info.path).map_err(|e| Error::InvalidFilename(e.into()))?)
            })
            .map(|filename| Ok(LayoutName::try_from_storage_name(filename?)?))
            .map(|layout_name| Ok(Layout::new(layout_name?)));
        Self {
            iter: Box::new(iter),
        }
    }
}

impl Iterator for ExtractLayouts<'_> {
    type Item = Result<Layout, Error>;
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

pub trait ExtractLayoutsIterator<'a>: Iterator<Item = LayoutInfo> + Sized + 'a {
    fn extract_layouts(self) -> ExtractLayouts<'a> {
        ExtractLayouts::new(self)
    }
}

impl<'a, I: Iterator<Item = LayoutInfo> + 'a> ExtractLayoutsIterator<'a> for I {}

#[derive(Debug)]
pub struct LayoutManager {
    layouts: Vec<Layout>,
}

impl LayoutManager {
    pub fn new(layouts: Vec<Layout>) -> Self {
        Self { layouts }
    }

    pub fn layout(&self, tmux_name: &str) -> Option<&Layout> {
        self.layouts
            .iter()
            .find(|layout| layout.tmux_name == tmux_name)
    }

    pub fn contains(&self, tmux_name: &str) -> bool {
        self.layouts
            .iter()
            .find(|s| s.tmux_name == tmux_name)
            .is_some()
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
    pub fn remove(&mut self, tmux_name: &str) -> Result<(), Error> {
        let exists = self.layouts.iter().any(|e| e.tmux_name == tmux_name);
        if !exists {
            return Err(Error::NotFound(tmux_name.to_owned()));
        }
        self.layouts.retain(|l| l.tmux_name != tmux_name);
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use color_eyre::Result;

    fn test_layout(name: &str) -> Result<Layout> {
        Ok(Layout::new(LayoutName::try_new(name.to_owned())?))
    }

    fn layout_manager_with_names(names: Vec<&'static str>) -> Result<LayoutManager> {
        let layouts = names
            .into_iter()
            .map(|name| test_layout(name))
            .collect::<Result<Vec<_>>>()?;
        Ok(LayoutManager::new(layouts))
    }

    #[test]
    fn layout() -> Result<()> {
        let layout_manager = layout_manager_with_names(vec!["test"])?;
        let layout = layout_manager.layout("test");
        assert_eq!(layout, Some(&test_layout("test")?));
        Ok(())
    }

    #[test]
    fn contains() -> Result<()> {
        let layout_manager = layout_manager_with_names(vec!["test1", "test2"])?;
        assert_eq!(layout_manager.contains("test1"), true);
        assert_eq!(layout_manager.contains("test2"), true);
        Ok(())
    }

    #[test]
    fn remove() -> Result<()> {
        let mut layout_manager = layout_manager_with_names(vec!["test"])?;
        layout_manager.remove("test")?;
        assert_eq!(layout_manager.contains("test"), false);
        Ok(())
    }

    mod create {
        use super::*;

        #[test]
        fn normal() -> Result<()> {
            let mut layout_manager = layout_manager_with_names(Vec::new())?;
            layout_manager.create(test_layout("test")?)?;
            assert_eq!(layout_manager.contains("test"), true);
            Ok(())
        }

        #[test]
        fn duplicate() -> Result<()> {
            let mut layout_manager = layout_manager_with_names(vec!["test"])?;
            let result = layout_manager.create(test_layout("test")?);
            assert_eq!(result.is_err(), true);
            Ok(())
        }
    }

    mod deduce_name {
        use super::*;

        #[test]
        fn normal() -> Result<()> {
            let layout_manager = layout_manager_with_names(Vec::new())?;
            let name = LayoutName::try_from_path(Path::new("/test/test"), &layout_manager)?;
            assert_eq!(name.tmux_name(), "test");
            Ok(())
        }

        #[test]
        fn simple_duplicate() -> Result<()> {
            let layout_manager = layout_manager_with_names(vec!["test"])?;
            let name = LayoutName::try_from_path(Path::new("/test/test"), &layout_manager)?;
            assert_eq!(name.tmux_name(), "test/test");
            Ok(())
        }

        #[test]
        fn undeducable_duplicate() -> Result<()> {
            let layout_manager = layout_manager_with_names(vec!["test"])?;
            let _ = LayoutName::try_from_path(Path::new("/test"), &layout_manager).unwrap_err();
            Ok(())
        }

        #[test]
        fn multiple() -> Result<()> {
            let layout_manager = layout_manager_with_names(vec!["test", "test/test"])?;
            let name = LayoutName::try_from_path(Path::new("/test/test/test"), &layout_manager)?;
            assert_eq!(name.tmux_name(), "test/test/test");
            Ok(())
        }
    }
}
