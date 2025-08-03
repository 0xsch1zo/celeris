use crate::utils;
use itertools::Itertools;
use std::{
    fmt::Display,
    iter,
    ops::ControlFlow,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub enum Error {
    AlreadyExists(String),
    NotFound(String),
    InvalidFilename(Box<dyn std::error::Error + Sync + Send>),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::NotFound(entry) => {
                format!("manifest entry not found: {entry}")
            }
            Self::AlreadyExists(entry) => {
                format!("manifest entry already exists: {entry}")
            }
            Self::InvalidFilename(_) => "failed to get a file name of path".to_owned(),
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
pub struct Entry {
    pub name: String,
    pub session_path: PathBuf,
    pub script_name: String,
}

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            || self.script_name == other.script_name
            || self.session_path == other.session_path
    }
}

impl Entry {
    pub fn new(name: String, session_path: PathBuf) -> Self {
        const DELIMETER: &str = "_";
        let script_name = name.replace("/", DELIMETER);
        Self {
            name,
            script_name,
            session_path,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn session_path(&self) -> &Path {
        self.session_path.as_path()
    }

    pub fn script_name(&self) -> &str {
        &self.script_name
    }
}

#[derive(Debug, Default)]
pub struct Manifest {
    pub entries: Vec<Entry>,
}

impl Manifest {
    pub fn deduce_name(&self, path: &Path) -> Result<String, Error> {
        if self.entries.iter().any(|e| e.session_path == path) {
            return Err(Error::AlreadyExists(path.to_string_lossy().to_string()));
        }

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

    pub fn extend(self, entry: Entry) -> Result<Self, Error> {
        if self.entries.contains(&entry) {
            return Err(Error::AlreadyExists(entry.name));
        }

        let entries = self.entries.into_iter().chain(iter::once(entry)).collect();
        Ok(Self { entries })
    }

    pub fn entry(&self, name: &str) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.name == name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.entries.iter().find(|s| s.name == name).is_some()
    }

    pub fn filter_out(self, name: &str) -> Result<Self, Error> {
        let exists = self.entries.iter().any(|e| e.name == name);
        if !exists {
            return Err(Error::NotFound(name.to_owned()));
        }

        let entries = self
            .entries
            .into_iter()
            .filter(|e| e.name != name)
            .collect_vec();
        Ok(Self { entries })
    }

    pub fn list(&self) -> Vec<&String> {
        self.entries.iter().map(|e| &e.name).collect::<Vec<_>>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use color_eyre::Result;

    fn test_entry(name: &str) -> Result<Entry> {
        Ok(Entry::new(name.to_owned(), PathBuf::new()))
    }

    fn manifest_with_names(names: Vec<&'static str>) -> Result<Manifest> {
        let entries = names
            .into_iter()
            .map(|name| test_entry(name))
            .collect::<Result<Vec<_>>>()?;
        let manifest = Manifest { entries };
        Ok(manifest)
    }

    #[test]
    fn entry() -> Result<()> {
        let manifest = manifest_with_names(vec!["test"])?;
        let entry = manifest.entry("test");
        assert_eq!(entry, Some(&test_entry("test")?));
        Ok(())
    }

    #[test]
    fn contains() -> Result<()> {
        let manifest = manifest_with_names(vec!["test1", "test2"])?;
        assert_eq!(manifest.contains("test1"), true);
        assert_eq!(manifest.contains("test2"), true);
        Ok(())
    }

    #[test]
    fn filter_out() -> Result<()> {
        let manifest = manifest_with_names(vec!["test"])?;
        let manifest = manifest.filter_out("test")?;
        assert_eq!(manifest.contains("test"), false);
        Ok(())
    }

    mod push {
        use super::*;

        #[test]
        fn normal() -> Result<()> {
            let manifest = manifest_with_names(Vec::new())?;
            let manifest = manifest.extend(test_entry("test")?)?;
            assert_eq!(manifest.contains("test"), true);
            Ok(())
        }

        #[test]
        fn duplicate() -> Result<()> {
            let manifest = manifest_with_names(vec!["test"])?;
            let result = manifest.extend(test_entry("test")?);
            assert_eq!(result.is_err(), true);
            Ok(())
        }
    }

    mod deduce_name {
        use super::*;

        #[test]
        fn normal() -> Result<()> {
            let manifest = manifest_with_names(Vec::new())?;
            let name = manifest.deduce_name(Path::new("/test/test"))?;
            assert_eq!(name, "test");
            Ok(())
        }

        #[test]
        fn simple_duplicate() -> Result<()> {
            let manifest = manifest_with_names(vec!["test"])?;
            let name = manifest.deduce_name(Path::new("/test/test"))?;
            assert_eq!(name, "test/test");
            Ok(())
        }

        #[test]
        fn undeducable_duplicate() -> Result<()> {
            let manifest = manifest_with_names(vec!["test"])?;
            let _ = manifest.deduce_name(Path::new("/test")).unwrap_err();
            Ok(())
        }

        #[test]
        fn multiple() -> Result<()> {
            let manifest = manifest_with_names(vec!["test", "test/test"])?;
            let name = manifest.deduce_name(Path::new("/test/test/test"))?;
            assert_eq!(name, "test/test/test");
            Ok(())
        }
    }
}
