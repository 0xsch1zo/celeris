pub mod search;

use crate::utils;
use git2::Repository;
use std::cell::RefCell;
use std::path::PathBuf;
use walkdir::DirEntry;

#[derive(Debug, Clone)]
pub struct Repo {
    pub name: String,
    pub path: PathBuf,
}

struct RepoManager {
    repos: Vec<RefCell<Repo>>,
}

impl RepoManager {
    fn new() -> Self {
        Self {
            repos: Vec::<RefCell<Repo>>::new(),
        }
    }

    pub fn push_if_repo(&mut self, entry: &DirEntry) -> bool {
        match Repository::open(entry.path()) {
            Ok(repo) if repo.workdir().is_some_and(|r| r == entry.path()) => {
                self.repos.push(RefCell::new(Repo {
                    name: utils::file_name(entry),
                    path: entry.path().to_path_buf(),
                }));
                self.deduplicate();
                false
            }
            _ => true,
        }
    }

    fn make_unique(duplicates: Vec<&RefCell<Repo>>) {
        const SEPARATOR: &str = "/";

        // stores the temporary paths of parents used to derive a unique name
        let mut parents: Vec<_> = duplicates.iter().map(|r| r.borrow().path.clone()).collect();

        while !utils::is_unique(duplicates.iter().map(|r| r.borrow().name.clone())) {
            duplicates.iter().enumerate().for_each(|(index, repo)| {
                let mut repo = repo.borrow_mut();
                let parent_dir = parents[index]
                    .parent()
                    .map_or_else(
                        || Some(String::from("/")),
                        |parent| Some(parent.file_name()?.to_string_lossy().to_string()),
                    )
                    .unwrap_or(String::from("/"));
                parents[index].pop();
                repo.name = format!("{}{}{}", parent_dir, SEPARATOR, repo.name)
            });
        }
    }

    fn deduplicate(&mut self) {
        self.repos.iter().for_each(|repo| {
            let duplicate_repo_names: Vec<_> = self
                .repos
                .iter()
                .filter(|other| repo.borrow().name == other.borrow().name)
                .collect();
            if duplicate_repo_names.is_empty() {
                return;
            }

            Self::make_unique(duplicate_repo_names);
        });
    }
}
