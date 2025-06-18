use crate::utils;
use git2::Repository;
use std::cell::{Ref, RefCell};
use std::path::PathBuf;
use walkdir::DirEntry;

#[derive(Debug, Clone)]
pub struct Session {
    pub name: String,
    pub path: PathBuf,
}

pub struct Sessions {
    sessions: Vec<RefCell<Session>>,
}

impl Sessions {
    pub fn new() -> Self {
        Self {
            sessions: Vec::<RefCell<Session>>::new(),
        }
    }

    fn make_unique(duplicates: Vec<&RefCell<Session>>) {
        const SEPARATOR: &str = "/";

        // stores the temporary paths of parents used to derive a unique name
        let mut parents: Vec<_> = duplicates
            .iter()
            .map(|session| session.borrow().path.clone())
            .collect();

        while !utils::is_unique(
            duplicates
                .iter()
                .map(|session| session.borrow().name.clone()),
        ) {
            duplicates.iter().enumerate().for_each(|(index, session)| {
                let mut session = session.borrow_mut();
                let parent_dir = parents[index]
                    .parent()
                    .map_or_else(
                        || Some(String::from("/")),
                        |parent| Some(parent.file_name()?.to_string_lossy().to_string()),
                    )
                    .unwrap_or(String::from("/"));
                parents[index].pop();
                session.name = format!("{}{}{}", parent_dir, SEPARATOR, session.name)
            });
        }
    }

    fn deduplicate(&mut self) {
        self.sessions.iter().for_each(|session| {
            let duplicate_sessions: Vec<_> = self
                .sessions
                .iter()
                .filter(|other| session.borrow().name == other.borrow().name)
                .collect();
            if duplicate_sessions.is_empty() {
                return;
            }

            Self::make_unique(duplicate_sessions);
        });
    }

    pub fn push_if_repo(&mut self, entry: &DirEntry) -> bool {
        match Repository::open(entry.path()) {
            Ok(repo) if repo.workdir().is_some_and(|r| r == entry.path()) => {
                self.sessions.push(RefCell::new(Session {
                    name: utils::file_name(entry),
                    path: entry.path().to_path_buf(),
                }));
                self.deduplicate();
                false
            }
            _ => true,
        }
    }

    pub fn get(&self) -> Vec<Ref<'_, Session>> {
        self.sessions
            .iter()
            .map(|session| session.borrow())
            .collect::<Vec<_>>()
    }
}
