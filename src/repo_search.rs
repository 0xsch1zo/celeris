use crate::{config::Config, utils};
use color_eyre::Result;
use color_eyre::owo_colors::OwoColorize;
use git2::Repository;
use std::path::{Path, PathBuf};
use walkdir::{DirEntry, WalkDir};

pub fn search(config: &Config) -> Result<Vec<String>> {
    if config.search_roots.is_empty() {
        eprintln!(
            "{}: search roots are not defined, nothing to search in",
            "warning".yellow().bold()
        );
        return Ok(Vec::new());
    }

    let mut repos: Vec<PathBuf> = Vec::new();
    // Side-effects were needed
    config.search_roots.iter().for_each(|root| {
        let local_excludes = root.excludes.clone().unwrap_or_default();

        let _: Vec<_> = WalkDir::new(&root.path)
            .max_depth(root.depth.unwrap_or(config.depth))
            .into_iter()
            .filter_entry(|entry| {
                if is_excluded_from(&config.excludes, entry)
                    || is_excluded_from(&local_excludes, entry)
                {
                    return false;
                }

                // There was no other way to do it using walkdir
                repos.push_if_repo(entry);
                config.search_subdirs || !is_repo(entry)
            })
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().is_dir())
            .collect();
    });

    Ok(repos
        .into_iter()
        .map(utils::shorten_path)
        .map(|p| utils::path_to_string(p.as_path()))
        .collect::<Result<Vec<_>>>()?)
}

fn is_excluded_from(excludes: &Vec<String>, entry: &DirEntry) -> bool {
    !excludes.iter().all(|exclude| !is_excluded(exclude, entry))
}

fn is_excluded(exclude: &str, entry: &DirEntry) -> bool {
    let exclude_path = Path::new(exclude);
    if exclude_path.is_absolute() {
        exclude_path == entry.path()
    } else {
        exclude == entry.file_name().to_str().unwrap_or_default()
    }
}

trait RepoPushExt {
    fn push_if_repo(&mut self, entry: &DirEntry);
}

impl RepoPushExt for Vec<PathBuf> {
    fn push_if_repo(&mut self, entry: &DirEntry) {
        if is_repo(entry) {
            self.push(entry.path().to_path_buf());
        }
    }
}

fn is_repo(entry: &DirEntry) -> bool {
    match Repository::open(entry.path()) {
        Ok(repo) if repo.workdir().is_some_and(|r| r == entry.path()) => true,
        _ => false,
    }
}
