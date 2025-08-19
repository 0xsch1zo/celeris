#[allow(dead_code)]
mod common;
use std::fs;

use celeris::Config;
use color_eyre::{Result, eyre::Context};

use common::TestDirectoryManager;

#[test]
fn config_file_default() -> Result<()> {
    let dir_mgr = TestDirectoryManager::new()?;
    let _config = Config::new(dir_mgr.as_ref());
    assert!(dir_mgr.config_dir().join("config.toml").exists());
    Ok(())
}

#[test]
fn validation() -> Result<()> {
    let dir_mgr = TestDirectoryManager::new()?;
    let config_path = dir_mgr.config_dir().join("config.toml");
    let config = format!(
        r#"
    [[search_roots]]
    path = {} 
        "#,
        dir_mgr.config_dir().join("doesntexist").to_string_lossy()
    );
    fs::write(&config_path, &config).wrap_err("failed to write test config")?;
    let _ = Config::new(dir_mgr.as_ref()).expect_err("config should detect non existing path");

    let config = format!(
        r#"
    [[search_roots]]
    path = {} 
        "#,
        config_path.to_string_lossy()
    );
    fs::write(&config_path, &config).wrap_err("failed to write test config")?;
    let _ = Config::new(dir_mgr.as_ref())
        .expect_err("config should detect a search root is a file and not a directory");
    Ok(())
}
