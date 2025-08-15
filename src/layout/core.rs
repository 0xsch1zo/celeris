use crate::utils;
use itertools::Itertools;
use sanitize_filename;
use std::{
    ffi::OsString,
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
    NotADirectory(PathBuf),
}

pub enum PathState {
    Directory,
    File,
}

pub enum TemplateDecision {
    GenerateCustom,
    GenerateDefault,
    LeaveEmpty,
}

pub enum EditorDecision {
    Spawn,
    DontSpawn,
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
            Self::NotADirectory(path) => {
                format!("the following path needs to be a directory: {path:?}")
            }
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

impl LayoutName {
    const STORAGE_NAME_DELIMETER: &str = ".";
    const TMUX_NAME_DELIMETER: &str = "/";

    pub fn try_new(name: String) -> Result<Self, Error> {
        let tmux_special_chars = ['@', '$', '%', ':', '.'];
        if name.chars().any(|c| tmux_special_chars.contains(&c)) {
            return Err(Error::InvalidLayoutName(format!(
                "name contains characters that tmux treats specially({tmux_special_chars:?}). You can also set a custom name when creating a session"
            )));
        }
        Ok(Self(name))
    }

    pub fn try_from_path(
        path: &Path,
        state: PathState,
        layout_manager: &LayoutManager,
    ) -> Result<Self, Error> {
        if let PathState::File = state {
            return Err(Error::NotADirectory(path.to_owned()));
        }
        let mut name = utils::file_name(&path).map_err(|e| Error::InvalidFilename(e.into()))?;
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
        let path = layouts_dir.join(&self.storage_name);
        // yeah it's ugly because add_extension is still in fucking nightly
        let final_extension = if path.extension().is_some() {
            let mut final_extension = path.extension().unwrap().to_owned();
            final_extension.push(".");
            final_extension.push(Self::extension());
            final_extension
        } else {
            Self::extension()
        };

        path.with_extension(final_extension)
    }

    pub fn extension() -> OsString {
        OsString::from("lua")
    }
}

pub struct LayoutInfo {
    path: PathBuf,
    state: PathState,
}

impl LayoutInfo {
    pub fn new(path: PathBuf, state: PathState) -> Self {
        Self { path, state }
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
            .filter(|info| match info.state {
                PathState::Directory => false,
                PathState::File => true,
            })
            .filter(|info| info.path.extension() == Some(&Layout::extension()))
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

pub fn template_decision(template_disabled: bool, custom_exists: bool) -> TemplateDecision {
    match (template_disabled, custom_exists) {
        (true, _) => TemplateDecision::LeaveEmpty,
        (_, true) => TemplateDecision::GenerateCustom,
        (_, false) => TemplateDecision::GenerateDefault,
    }
}

pub fn editor_decision(disable_editor_on_creation: bool) -> EditorDecision {
    match disable_editor_on_creation {
        true => EditorDecision::DontSpawn,
        false => EditorDecision::Spawn,
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
    fn storage_path() -> Result<()> {
        let layout_mgr = layout_manager_with_names(vec!["aaa", "ccc"])?;
        let layouts = ["/test/aaa", "/test/dd.d/bbb"]
            .into_iter()
            .map(|path| {
                LayoutName::try_from_path(Path::new(path), PathState::Directory, &layout_mgr)
            })
            .map(|layout_name| Ok(Layout::new(layout_name?)))
            .collect::<Result<Vec<_>, Error>>()?;
        let layout_dir = PathBuf::from("/test");
        // dummy is because there is no goddamn add_extension
        let expected_storage_names = ["test.aaa.dummy", "bbb"];
        let expected_storage_paths = expected_storage_names
            .into_iter()
            .map(|name| layout_dir.join(name).with_extension(Layout::extension()))
            .collect_vec();
        let storage_paths_got = layouts
            .iter()
            .map(|layout| layout.storage_path(&layout_dir))
            .collect_vec();

        assert_eq!(expected_storage_paths, storage_paths_got);
        Ok(())
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
            let name = LayoutName::try_from_path(
                Path::new("/test/test"),
                PathState::Directory,
                &layout_manager,
            )?;
            assert_eq!(name.tmux_name(), "test");
            Ok(())
        }

        #[test]
        fn simple_duplicate() -> Result<()> {
            let layout_manager = layout_manager_with_names(vec!["test"])?;
            let name = LayoutName::try_from_path(
                Path::new("/test/test"),
                PathState::Directory,
                &layout_manager,
            )?;
            assert_eq!(name.tmux_name(), "test/test");
            Ok(())
        }

        #[test]
        fn undeducable_duplicate() -> Result<()> {
            let layout_manager = layout_manager_with_names(vec!["test"])?;
            let _ = LayoutName::try_from_path(
                Path::new("/test"),
                PathState::Directory,
                &layout_manager,
            )
            .unwrap_err();
            Ok(())
        }

        #[test]
        fn multiple() -> Result<()> {
            let layout_manager = layout_manager_with_names(vec!["test", "test/test"])?;
            let name = LayoutName::try_from_path(
                Path::new("/test/test/test"),
                PathState::Directory,
                &layout_manager,
            )?;
            assert_eq!(name.tmux_name(), "test/test/test");
            Ok(())
        }
    }
}
