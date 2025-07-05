use crate::config::Config;
use crate::manifest;
use crate::manifest::Manifest;
use crate::repos::Repo;
use crate::script;
use color_eyre::Result;
use color_eyre::eyre::OptionExt;
use color_eyre::eyre::WrapErr;
use color_eyre::eyre::eyre;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{self, Stdio};

#[derive(Clone)]
pub struct SessionProperties {
    pub name: String,
    pub path: PathBuf,
}

impl From<Repo> for SessionProperties {
    fn from(value: Repo) -> Self {
        Self {
            name: value.name,
            path: value.path,
        }
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

    pub fn create(&mut self, props: SessionProperties) -> Result<()> {
        let name = props.name.clone();
        self.manifest.push_unique(props)?;
        let entry = self.entry(&name)?;
        script::edit(&entry, &self.config)?;
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
        self.manifest.remove(name)?;
        Ok(())
    }
}

pub struct NameFilter {
    handle: process::Child,
}

impl NameFilter {
    pub fn spawn(config: &Config) -> Result<Self> {
        let handle = process::Command::new(config.filter_command.program.clone())
            .args(config.filter_command.args.clone())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .wrap_err("couldn't execute filter command")?;
        Ok(Self { handle })
    }

    pub fn filter(mut self, names: &[String]) -> Result<String> {
        let names = names.join("\n");
        self.handle
            .stdin
            .take()
            .ok_or_eyre("stdin of filter not available")?
            .write(&names.into_bytes())
            .wrap_err("failed to write to stdin of the filter command")?;
        let status = self
            .handle
            .wait()
            .wrap_err("filter is not running even though it was spawned")?;
        if !status.success() {
            let mut output = String::new();
            self.handle
                .stderr
                .take()
                .ok_or_eyre("stderr of filter not available when failing")?
                .read_to_string(&mut output)
                .wrap_err("failed to read stderr of filter when failing")?;
            return Err(eyre!("filter_command failed with: {output}"));
        }

        let mut output = String::new();
        self.handle
            .stdout
            .take()
            .ok_or_eyre("stdout of filter not available")?
            .read_to_string(&mut output)
            .wrap_err("failed to read stdin of filter")?;
        Ok(output.trim().to_string())
    }
}
