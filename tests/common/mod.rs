use celeris::{
    config::Config, directory_manager::DirectoryManager, session_manager::SessionManager,
};
use color_eyre::{Result, eyre::Context};
use delegate::delegate;
use itertools::Itertools;
use std::{
    env,
    fs::{self, File},
    path::PathBuf,
    sync::Arc,
};

pub struct TestDirectoryManager(Arc<DirectoryManager>);

impl TestDirectoryManager {
    fn testing_dir() -> PathBuf {
        env::temp_dir().join("celeris_test")
    }

    pub fn new() -> Result<Self> {
        let testing_dir = Self::testing_dir();
        fs::create_dir(&testing_dir).wrap_err("failed to create main test dir")?;

        let cache_dir = testing_dir.join("cache");
        fs::create_dir(&cache_dir).wrap_err("failed to create cache dir")?;

        let config_dir = testing_dir.join("config");
        fs::create_dir(&config_dir).wrap_err("failed to create config dir")?;

        let mut directory_manager = DirectoryManager::new();
        directory_manager
            .set_cache_dir(cache_dir)?
            .set_config_dir(config_dir)?;

        let dir_mgr = Self(Arc::new(directory_manager));

        let repo_dir = dir_mgr.repo_dir();
        fs::create_dir(&repo_dir).wrap_err("failed to create repo dir")?;

        Ok(dir_mgr)
    }

    pub fn custom_template_path(&self) -> Result<PathBuf> {
        Ok(self.config_dir()?.join("template").with_extension("lua"))
    }

    pub fn repo_dir(&self) -> PathBuf {
        Self::testing_dir().join("repos")
    }

    pub fn inner(&self) -> &Arc<DirectoryManager> {
        &self.0
    }

    delegate! {
        to self.0 {
            #[expr(Ok($?))]
            pub fn config_dir(&self) -> Result<PathBuf>;
            #[expr(Ok($?))]
            pub fn layouts_dir(&self) -> Result<PathBuf>;
        }
    }
}

impl Drop for TestDirectoryManager {
    fn drop(&mut self) {
        fs::remove_dir_all(Self::testing_dir()).expect("Failed to remove testing directory")
    }
}

impl AsRef<DirectoryManager> for TestDirectoryManager {
    fn as_ref(&self) -> &DirectoryManager {
        &self.0
    }
}

pub fn test_session_manager(dir_mgr: Arc<DirectoryManager>) -> Result<SessionManager> {
    let config = Arc::new(Config::default());
    Ok(SessionManager::new(config, dir_mgr)?)
}

pub fn create_dummy_layouts(names: &[&str], dir_mgr: &DirectoryManager) -> Result<()> {
    let _: Vec<_> = names
        .into_iter()
        .map(|name| -> Result<File> {
            Ok(File::create_new(
                dir_mgr.layouts_dir()?.join(name).with_extension("lua"),
            )?)
        })
        .try_collect()?;
    Ok(())
}

pub fn new_layout(
    layout_name: &str,
    layout_contents: &str,
    dir_mgr: &DirectoryManager,
) -> Result<()> {
    let layout_path = dir_mgr
        .layouts_dir()?
        .join(layout_name)
        .with_extension("lua");
    fs::write(layout_path, layout_contents).wrap_err("failed to write layout contents")?;
    Ok(())
}
