#[allow(dead_code)]
mod common;

use crate::common::TestDirectoryManager;
use celeris::{Config, SearchRoot};
use color_eyre::Result;
use color_eyre::eyre::Context;
use git2::Repository;
use itertools::Itertools;
use std::iter;
use std::path::Path;
use std::{fs, path::PathBuf};

fn create_repos(root: &Path, dirs: &[String]) -> Result<()> {
    dirs.iter().try_for_each(|dir| -> Result<()> {
        fs::create_dir(root.join(dir))?;
        Repository::init(root.join(dir))?;
        Ok(())
    })?;
    Ok(())
}

fn basic_config(search_root: SearchRoot) -> Config {
    let config = Config::default();
    Config {
        search_roots: vec![search_root],
        ..config
    }
}

#[test]
fn basic_search() -> Result<()> {
    let dir_mgr = TestDirectoryManager::new()?;

    let search_root = SearchRoot {
        path: dir_mgr.repo_dir().to_string_lossy().to_string(),
        depth: None,
        excludes: None,
    };

    let targets = ["test1", "test21", "test-123_"]
        .into_iter()
        .map(ToOwned::to_owned)
        .sorted()
        .collect_vec();
    create_repos(Path::new(&search_root.path), &targets)?;
    let config = basic_config(search_root);
    let results = celeris::search(&config)?
        .into_iter()
        .map(PathBuf::from)
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .sorted()
        .collect_vec();
    assert_eq!(results, targets);
    Ok(())
}

#[test]
fn search_with_config() -> Result<()> {
    let dir_mgr = TestDirectoryManager::new()?;
    let config_path = dir_mgr.config_dir().join("config.toml");
    let config = format!(
        r#"
[[search_roots]]
path = "{}"
"#,
        dir_mgr.repo_dir().to_string_lossy()
    );
    fs::write(config_path, config.as_bytes()).wrap_err("failed to write test config")?;
    let config = Config::new(dir_mgr.as_ref())?;

    let given_repos = ["test1", "test2", "test3"]
        .map(ToOwned::to_owned)
        .map(|repo_name| dir_mgr.repo_dir().join(repo_name));

    given_repos
        .iter()
        .map(|path| {
            Repository::init(path)?;
            Ok(())
        })
        .collect::<Result<()>>()?;

    let repos = celeris::search(&config)?;
    let repos = repos.into_iter().map(PathBuf::from).collect_vec();
    assert_eq!(given_repos.iter().all(|r| repos.contains(r)), true);
    Ok(())
}

#[test]
fn search_nested() -> Result<()> {
    let dir_mgr = TestDirectoryManager::new()?;

    let search_root = SearchRoot {
        path: dir_mgr.repo_dir().to_string_lossy().to_string(),
        depth: None,
        excludes: None,
    };

    let repo_names = ["sadfqwer", "foo", "bar"]
        .into_iter()
        .map(ToOwned::to_owned)
        .collect_vec();
    let roots = ["test1", "test2", "test3"].map(|root| Path::new(&search_root.path).join(root));

    roots.iter().try_for_each(|root| fs::create_dir(root))?;
    roots
        .iter()
        .try_for_each(|root| create_repos(root, &repo_names))?;
    let repos = roots
        .into_iter()
        .map(|root| repo_names.iter().map(move |name| root.join(name)))
        .flatten()
        .sorted()
        .collect_vec();

    let config = basic_config(search_root);
    let results = celeris::search(&config)?
        .into_iter()
        .map(PathBuf::from)
        .sorted()
        .collect_vec();

    assert_eq!(repos, results);
    Ok(())
}

#[test]
fn custom_depth() -> Result<()> {
    let dir_mgr = TestDirectoryManager::new()?;

    let repo_root = dir_mgr.repo_dir().join("nested");
    fs::create_dir(&repo_root)?;

    let repos = ["test1", "test2", "test3"]
        .into_iter()
        .map(ToOwned::to_owned)
        .collect_vec();

    let search_root = SearchRoot {
        path: repo_root.to_string_lossy().to_string(),
        depth: Some(1),
        excludes: None,
    };

    let config_custom_depth = basic_config(search_root.clone());

    let custom_depth_results = celeris::search(&config_custom_depth)?
        .into_iter()
        .map(PathBuf::from)
        .map(|repo| repo.file_name().unwrap().to_str().unwrap().to_owned())
        .sorted()
        .collect_vec();
    assert_eq!(custom_depth_results, Vec::<String>::new());

    let search_root = SearchRoot {
        depth: Some(2),
        ..search_root
    };

    create_repos(Path::new(&search_root.path), &repos)?;

    let config_custom_depth = basic_config(search_root);

    let custom_depth_results = celeris::search(&config_custom_depth)?
        .into_iter()
        .map(PathBuf::from)
        .map(|repo| repo.file_name().unwrap().to_str().unwrap().to_owned())
        .sorted()
        .collect_vec();
    assert_eq!(custom_depth_results, repos);
    Ok(())
}

#[test]
fn search_subdirs() -> Result<()> {
    let dir_mgr = TestDirectoryManager::new()?;

    let repo_root = dir_mgr.repo_dir().join("nested");
    fs::create_dir(&repo_root)?;
    Repository::init(&repo_root)?;

    let repos = ["test1", "test2", "test3"]
        .into_iter()
        .map(ToOwned::to_owned)
        .collect_vec();

    create_repos(&repo_root, &repos)?;

    let search_root = SearchRoot {
        path: repo_root.to_string_lossy().to_string(),
        depth: None,
        excludes: None,
    };

    let config = basic_config(search_root.clone());

    let config_subdirs = Config {
        search_subdirs: true,
        ..config.clone()
    };

    let subdirs_results = celeris::search(&config_subdirs)?
        .into_iter()
        .map(PathBuf::from)
        .map(|repo| repo.file_name().unwrap().to_str().unwrap().to_owned())
        .sorted()
        .collect_vec();
    assert_eq!(
        subdirs_results,
        repos
            .into_iter()
            .chain(iter::once("nested".to_owned()))
            .sorted()
            .collect_vec(),
    );
    Ok(())
}

#[test]
fn excludes() -> Result<()> {
    let dir_mgr = TestDirectoryManager::new()?;

    let search_root = SearchRoot {
        path: dir_mgr.repo_dir().to_string_lossy().to_string(),
        depth: None,
        excludes: Some(vec!["test21".to_owned()]),
    };

    let targets = ["test1", "test21", "test-123_"]
        .into_iter()
        .map(ToOwned::to_owned)
        .sorted()
        .collect_vec();

    create_repos(Path::new(&search_root.path), &targets)?;

    let config = basic_config(search_root);
    let results = celeris::search(&config)?
        .into_iter()
        .map(PathBuf::from)
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .sorted()
        .collect_vec();
    assert_eq!(
        results,
        targets
            .clone()
            .into_iter()
            .filter(|t| *t != "test21")
            .collect_vec()
    );

    let search_root = SearchRoot {
        path: dir_mgr.repo_dir().to_string_lossy().to_string(),
        depth: None,
        excludes: None,
    };

    let config = basic_config(search_root);
    let config = Config {
        excludes: vec!["test1".to_owned()],
        ..config
    };
    let results = celeris::search(&config)?
        .into_iter()
        .map(PathBuf::from)
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .sorted()
        .collect_vec();
    assert_eq!(
        results,
        targets
            .clone()
            .into_iter()
            .filter(|t| *t != "test1")
            .collect_vec()
    );

    let search_root = SearchRoot {
        path: dir_mgr.repo_dir().to_string_lossy().to_string(),
        depth: None,
        excludes: Some(vec!["test21".to_owned()]),
    };

    let config = basic_config(search_root);
    let config = Config {
        excludes: vec!["test1".to_owned()],
        ..config
    };
    let results = celeris::search(&config)?
        .into_iter()
        .map(PathBuf::from)
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .sorted()
        .collect_vec();
    assert_eq!(results, vec!["test-123_"]);
    Ok(())
}
