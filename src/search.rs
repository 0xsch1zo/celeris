use crate::config::Config;
use git2::Repository;
use std::path::Path;
use walkdir::{DirEntry, WalkDir};

pub fn search(config: &Config) {
    let global_excludes = config
        .exclude_directories
        .clone() // for sanity purposes
        .unwrap_or(Vec::<String>::new());

    let repos = config
        .search_roots
        .iter()
        .map(|root| {
            WalkDir::new(&root.path)
                .into_iter()
                .filter_entry(|entry| {
                    global_excludes
                        .iter()
                        .all(|exclude| !is_excluded(exclude, entry))
                })
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.path().is_dir())
                .filter(|entry| match Repository::open(entry.path()) {
                    Ok(repo) => !repo.is_path_ignored(entry.path()).unwrap_or(true),
                    Err(_) => false,
                })
                .map(|entry| entry.path().to_str().unwrap().to_string())
                .collect::<Vec<_>>()
        })
        .flatten()
        .collect::<Vec<_>>();

    repos.iter().for_each(|repo| println!("{repo}"));
}

fn is_excluded(exclude: &str, entry: &DirEntry) -> bool {
    let exclude_path = Path::new(exclude);
    if exclude_path.is_absolute() {
        exclude_path == entry.path()
    } else {
        exclude == entry.file_name().to_str().unwrap_or_default()
    }
}
