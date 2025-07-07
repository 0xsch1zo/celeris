use color_eyre::eyre::{Context, OptionExt};
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

pub fn expand_path(mut path: PathBuf) -> Result<PathBuf> {
    if path.starts_with("~") {
        let home = dirs::home_dir()
            .ok_or_eyre("home directory not found despite home shell expansion used")
            .wrap_err("failed to expand ~ sign")?;
        let stripped_path = path.strip_prefix("~").wrap_err("failed to expand ~ sign")?;
        path = home.join(stripped_path);
    }
    Ok(path)
}
