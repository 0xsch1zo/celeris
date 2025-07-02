use crate::config::Config;
use crate::manifest::Manifest;
use crate::repos::Repo;
use crate::script;
use color_eyre::Result;
use color_eyre::eyre::OptionExt;
use color_eyre::eyre::WrapErr;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{self, Stdio};

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

pub fn create(manifest: &mut Manifest, config: &Config, props: SessionProperties) -> Result<()> {
    let name = props.name.clone();
    manifest.push_unique(props)?;
    let entry = manifest.entry(&name)?;
    script::edit(&script::path(&entry.hash)?, &config)?;
    Ok(())
}

pub fn filter_names(config: &Config, names: &[String]) -> Result<String> {
    let names = names.join("\n");
    let mut handle = process::Command::new(config.filter_command.program.clone())
        .args(config.filter_command.args.clone())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .wrap_err("couldn't execute filter command")?;

    handle
        .stdin
        .take()
        .ok_or_eyre("stdin of filter not available")?
        .write(&names.into_bytes())
        .wrap_err("failed to write to stding of the filter command")?;
    handle
        .wait()
        .wrap_err("filter is not running even though it was spawned")?;

    let mut output = String::new();
    handle
        .stdout
        .take()
        .ok_or_eyre("stdout of filter not available")?
        .read_to_string(&mut output)
        .wrap_err("failed to read stdin of filter")?;
    Ok(output.trim().to_string())
}
