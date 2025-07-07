use crate::config::Config;
use crate::manifest;
use crate::manifest::Manifest;
use crate::script;
use crate::utils;
use color_eyre::Result;
use color_eyre::eyre::OptionExt;
use color_eyre::eyre::WrapErr;
use std::path::PathBuf;

#[derive(Clone)]
pub enum Name {
    Deduced,
    Custom(String),
}

#[derive(Clone)]
pub struct SessionProperties {
    pub name: Name,
    pub path: PathBuf,
}

impl SessionProperties {
    pub fn new(name: Name, path: PathBuf) -> Self {
        Self { name, path }
    }

    pub fn from(name: Option<String>, path: PathBuf) -> Self {
        let name = match name {
            Some(name) => Name::Custom(name),
            None => Name::Deduced,
        };
        Self { name, path }
    }

    fn name(&self, manifest: &Manifest) -> Result<String> {
        Ok(match &self.name {
            Name::Custom(name) => name.to_owned(),
            Name::Deduced => manifest.deduce_name(&self.path).wrap_err(format!(
                "failed to deduce name of session with path: {}",
                self.path.display()
            ))?,
        })
    }
}

pub struct SessionManager<'a> {
    manifest: Manifest,
    config: &'a Config,
}

impl<'a> SessionManager<'a> {
    pub fn new(config: &'a Config) -> Result<Self> {
        Ok(Self {
            manifest: Manifest::new()?,
            config,
        })
    }

    fn entry(&self, name: &str) -> Result<&manifest::Entry> {
        Ok(self
            .manifest
            .entry(&name)
            .ok_or_eyre(format!("session not found: {}", name))?)
    }

    pub fn create(&mut self, mut props: SessionProperties) -> Result<()> {
        props.path = utils::expand_path(props.path)?;
        let name = props.name(&self.manifest)?;
        let entry = manifest::Entry::new(name.clone(), props.path)
            .wrap_err("failed to create session entry")?;
        self.manifest
            .push(entry)
            .wrap_err("failed to add session")?;
        let entry = self.entry(&name)?; // only ref
        script::edit(entry, &self.config)?;
        Ok(())
    }

    pub fn edit(&self, name: &str) -> Result<()> {
        let entry = self.entry(name)?;
        script::edit(entry, &self.config)?;
        Ok(())
    }

    pub fn run(&self, name: &str) -> Result<()> {
        let entry = self.entry(name)?;
        script::run(entry).wrap_err("script error")?;
        Ok(())
    }

    pub fn exists(&self, name: &str) -> bool {
        self.manifest.contains(name)
    }

    pub fn remove(&mut self, name: &str) -> Result<()> {
        self.manifest
            .remove(name)
            .wrap_err("failed to remove session")?;
        Ok(())
    }
}
