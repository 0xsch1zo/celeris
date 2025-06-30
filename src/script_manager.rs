use crate::internals_dir::internals_dir;
use crate::manifest::Manifest;
use crate::session_manager::SessionProperties;
use crate::utils;
use color_eyre::Result;
use color_eyre::eyre::Context;
use std::fs;
use std::path::PathBuf;
use std::process;

fn scripts_path() -> Result<PathBuf> {
    const SCRIPTS_DIR: &'static str = "scripts";
    let path = internals_dir()?.join(SCRIPTS_DIR);
    if !path.exists() {
        fs::create_dir(&path)
            .wrap_err_with(|| format!("failed to create scripts dir at {path:?}"))?;
    }
    Ok(path)
}

pub fn edit_script(manifest: &mut Manifest, props: SessionProperties) -> Result<()> {
    let name = props.name.clone();
    manifest.push_unique(props)?;
    let entry = manifest.entry(&name)?;
    let script_path = scripts_path()?.join(&entry.hash).with_extension("rhai");
    process::Command::new("nvim")
        .arg(utils::path_to_string(&script_path)?)
        .status()?;
    Ok(())
}
