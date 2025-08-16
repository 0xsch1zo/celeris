use color_eyre::Result;
use color_eyre::eyre::Context;
use color_eyre::eyre::eyre;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

const PROJECT_DIR_NAME: &'static str = "celeris";

pub struct DirectoryManagerBuilder {
    custom_config_path: Option<PathBuf>,
    custom_cache_path: Option<PathBuf>,
}

impl DirectoryManagerBuilder {
    pub fn new() -> Self {
        Self {
            custom_config_path: None,
            custom_cache_path: None,
        }
    }

    fn check_path(path: &Path, directory_type: &str) -> Result<()> {
        if !path.exists() {
            return Err(eyre!(
                "custom {directory_type} directory not found: {:?}",
                path
            ));
        }
        Ok(())
    }

    pub fn config_dir(&mut self, path: PathBuf) -> Result<&mut Self> {
        Self::check_path(&path, "config")?;
        self.custom_config_path = Some(path);
        Ok(self)
    }

    pub fn cache_dir(&mut self, path: PathBuf) -> Result<&mut Self> {
        Self::check_path(&path, "cache")?;
        self.custom_cache_path = Some(path);
        Ok(self)
    }

    pub fn build(&mut self) -> Result<DirectoryManager> {
        DirectoryManager::try_new(
            self.custom_config_path.take(),
            self.custom_cache_path.take(),
        )
    }
}

#[derive(Debug)]
pub struct DirectoryManager {
    config_dir: PathBuf,
    cache_dir: PathBuf,
    layouts_dir: PathBuf,
}

impl DirectoryManager {
    pub fn builder() -> DirectoryManagerBuilder {
        DirectoryManagerBuilder::new()
    }

    fn try_new(
        custom_config_path: Option<PathBuf>,
        custom_cache_path: Option<PathBuf>,
    ) -> Result<Self> {
        let config_dir = Self::config_dir_init(custom_config_path)?;
        let layouts_dir = Self::layouts_dir_init(&config_dir)?;
        let cache_dir = Self::cache_dir_init(custom_cache_path)?;
        Ok(Self {
            config_dir,
            layouts_dir,
            cache_dir,
        })
    }

    fn config_dir_init(custom_config_path: Option<PathBuf>) -> Result<PathBuf> {
        let default_path = dirs::config_dir();
        let default_path = default_path.as_ref();
        let path = match (custom_config_path.as_ref(), default_path) {
            (Some(path), _) => path.to_owned(),
            (None, Some(default_path)) => {
                let path = default_path.join(PROJECT_DIR_NAME);
                if !path.exists() {
                    fs::create_dir(&path)
                        .wrap_err_with(|| format!("failed to create config directory: {path:?}"))?;
                }
                path
            }
            (None, None) => {
                return Err(eyre!(
                    "local config directory not found, pass -c/--config to set custom config path",
                ));
            }
        };
        Ok(path)
    }

    fn layouts_dir_init(config_dir: &Path) -> Result<PathBuf> {
        const LAYOUTS_DIR: &'static str = "layouts";
        let layouts_dir = config_dir.join(LAYOUTS_DIR);
        if !layouts_dir.exists() {
            fs::create_dir(&layouts_dir)
                .wrap_err_with(|| format!("failed to create layouts dir: {layouts_dir:?}"))?;
        }
        Ok(layouts_dir)
    }

    fn cache_dir_init(custom_cache_path: Option<PathBuf>) -> Result<PathBuf> {
        let default_path = dirs::cache_dir();
        let default_path = default_path.as_ref();
        let path = match (custom_cache_path.as_ref(), default_path) {
            (Some(path), _) => path.to_owned(),
            (None, Some(default_path)) => {
                let path = default_path.join(PROJECT_DIR_NAME);
                if !path.exists() {
                    fs::create_dir(&path)
                        .wrap_err_with(|| format!("failed to create cache directory: {path:?}"))?;
                }
                path
            }
            (None, None) => {
                return Err(eyre!(
                    "local cache directory not found, pass -a/--cache-dir to set custom cache path"
                ));
            }
        };
        Ok(path)
    }

    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    pub fn layouts_dir(&self) -> &Path {
        &self.layouts_dir
    }
}
