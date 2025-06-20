use color_eyre::eyre::{Context, OptionExt, eyre};
use color_eyre::{self, Result};
use std::collections::HashSet;
use std::hash::Hash;
use std::path::{Path, PathBuf};
use walkdir::DirEntry;

pub fn is_unique<T>(iter: T) -> bool
where
    T: IntoIterator,
    T::Item: Eq + Hash,
{
    let mut uniq = HashSet::new();
    iter.into_iter().all(move |x| uniq.insert(x))
}

pub fn file_name(entry: &DirEntry) -> String {
    entry.file_name().to_string_lossy().to_string()
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
        .ok_or_eyre(format!("Invalid utf-8 encoding of path: {path:?}"))?
        .to_string())
}
