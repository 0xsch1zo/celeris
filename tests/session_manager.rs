mod common;

use color_eyre::{Result, eyre::Context};
use common::TestDirectoryManager;
use git2::Repository;
use sesh::{config::Config, repo_search, session_manager::ListSessionsOptions};
use std::{fs, rc::Rc};

#[test]
fn list_sessions() -> Result<()> {
    let dir_mgr = TestDirectoryManager::new()?;
    let dummy_layouts = ["test1", "test2", "test3"];
    common::create_dummy_layouts(&dummy_layouts, dir_mgr.as_ref())?;
    let session_manager = common::test_session_manager(Rc::clone(dir_mgr.inner()))?;

    let opts = ListSessionsOptions {
        tmux_format: false,
        include_active: false,
        exclude_running: true,
    };
    let output = session_manager.list(opts)?;
    output
        .lines()
        .map(str::trim)
        .zip(dummy_layouts)
        .for_each(|(output, session)| assert_eq!(output, session));
    Ok(())
}

#[test]
fn remove_session() -> Result<()> {
    let dir_mgr = TestDirectoryManager::new()?;
    common::create_dummy_layouts(&["test"], dir_mgr.as_ref())?;
    let session_manager = common::test_session_manager(Rc::clone(dir_mgr.inner()))?;
    let layout_path = dir_mgr
        .as_ref()
        .layouts_dir()?
        .join("test")
        .with_extension("lua");

    assert!(layout_path.exists());
    session_manager.remove("test")?;
    assert!(!layout_path.exists());

    Ok(())
}

#[test]
fn find_repos() -> Result<()> {
    let dir_mgr = TestDirectoryManager::new()?;
    let config_path = dir_mgr.config_dir()?.join("config.toml");
    let config = format!(
        r#"
[[search_roots]]
path = "{}"
"#,
        dir_mgr.repo_dir().to_string_lossy()
    );
    fs::write(config_path, config.as_bytes()).wrap_err("failed to write test config")?;
    let config = Config::new(dir_mgr.as_ref())?;

    let given_repos = ["test1", "test2", "test3"].map(ToOwned::to_owned);
    given_repos
        .iter()
        .map(|repo_name| dir_mgr.repo_dir().join(repo_name))
        .map(|path| {
            Repository::init(path)?;
            Ok(())
        })
        .collect::<Result<()>>()?;

    let repos = repo_search::search(&config)?;
    println!("{}", repos.join("\n"));
    assert_eq!(repos.iter().eq(given_repos.iter()), true);
    Ok(())
}
