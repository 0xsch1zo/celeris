use color_eyre::eyre::{Context, OptionExt, eyre};
use color_eyre::{self, Result};
use std::path::{Path, PathBuf};

pub fn file_name(path: &Path) -> Result<String> {
    Ok(path
        .file_name()
        .ok_or_eyre(format!("invalid path format {path:?}"))?
        .to_str()
        .ok_or_eyre(format!("invalid utf08 encoding path: {path:?}"))?
        .to_string())
}

// Consider moving back to config
pub fn config_dir() -> Result<PathBuf> {
    const CONFIG_DIR: &'static str = "sesh";
    let config_path: PathBuf = dirs::config_dir()
        .ok_or(eyre!("Couldn't find config dir to look for config"))?
        .join(CONFIG_DIR);

    Ok(config_path
        .canonicalize()
        .wrap_err("Main sesh config not found")?)
}

pub fn path_to_string(path: &Path) -> Result<String> {
    Ok(path
        .to_str()
        .ok_or_eyre(format!("invalid utf-8 encoding of path: {path:?}"))?
        .to_string())
}

pub fn shorten_path_string(path: &Path) -> Result<String> {
    let path = match dirs::home_dir() {
        Some(home) if path.starts_with(&home) => path.strip_prefix(home).unwrap(),
        _ => path,
    };
    Ok("~/".to_owned()
        + path
            .to_str()
            .ok_or_eyre(format!("Invalid utf-8 encoding of path: {path:?}"))?)
}
