use crate::utils;
use color_eyre::Result;
use color_eyre::eyre::Context;
use std::fs;
use std::path::PathBuf;

// Yes just this. It's shared between packages so decided to put this here
pub fn internals_dir() -> Result<PathBuf> {
    const INTERNALS_DIR: &'static str = "internals";
    let path = utils::config_dir()?.join(INTERNALS_DIR);
    if !path.exists() {
        fs::create_dir(&path)
            .wrap_err_with(|| format!("failed to create internals dir: {path:?}"))?;
    }
    Ok(path)
}

