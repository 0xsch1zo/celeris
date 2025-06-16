use crate::config::Config;
use crate::utils::is_unique;
use git2::Repository;
use std::ops::Index;
use std::path::Path;
use std::{io, path::PathBuf};
use walkdir::{DirEntry, WalkDir};

struct Session {
    name: String,
    path: PathBuf,
}

struct Sessions {
    sessions: Vec<Session>,
}

impl Sessions {
    fn make_unique(duplicates: Vec<&mut Session>) {
        const SEPARATOR: &str = "/";

        let mut dups: Vec<_> = duplicates
            .iter()
            .map(|d| Session {
                name: d.name.clone(),
                path: d.path.clone(),
            })
            .collect();
        while !is_unique(dups.iter().map(|d| d.name.clone())) {
            for ele in &mut dups {
                let parent_dir = ele
                    .path
                    .parent()
                    .map_or_else(
                        || Some(String::from("/")),
                        |parent| Some(parent.file_name()?.to_string_lossy().to_string()),
                    )
                    .unwrap_or(String::from("../"));
                ele.path.pop();
                ele.name = format!("{}{}{}", parent_dir, SEPARATOR, ele.name)
            }
        }
    }

    fn deduplicate(&mut self) {
        // Holds each duplicate name in it's own vector
        let mut duplicate_indicies = Vec::<Vec<usize>>::new();
        self.sessions.iter().for_each(|session| {
            let duplicate_sessions: Vec<_> = self
                .sessions
                .iter()
                .enumerate()
                .filter_map(|(index, other)| {
                    if session.name != other.name {
                        Some(index)
                    } else {
                        None
                    }
                })
                .collect();
            if duplicate_sessions.is_empty() {
                return;
            }

            duplicate_indicies.push(duplicate_sessions);
        });

        /*let mut duplicates: Vec<_> = duplicate_indicies
        .into_iter()
        .map(|duplicate_sessions| {
            duplicate_sessions
                .into_iter()
                .map(|index| {
                    self.sessions
                        .iter_mut()
                        .enumerate()
                        .filter(|(other_index, _)| *other_index == index)
                        .map(|(_, session)| session)
                        .collect::<Vec<&mut Session>>()
                })
                .collect::<Vec<_>>()
        })
        .collect();*/
        for dups in duplicate_indicies {
            let mut duplicates = Vec::<&mut Session>::new();
            for index in dups {
                duplicates.push(&mut self.sessions[index]);
            }
            Self::make_unique(duplicates);
        }
        // duplicates.push(duplicate_sessions);
        //});
    }

    fn push_if_repo(&mut self, entry: &DirEntry) -> bool {
        match Repository::open(entry.path()) {
            Ok(repo) if repo.workdir().is_some_and(|r| r == entry.path()) => {
                self.sessions.push(Session {
                    name: file_name(entry),
                    path: entry.path().to_path_buf(),
                });
                false
            }
            _ => true,
        }
    }
}

pub fn search(config: &Config) -> Result<Vec<String>, io::Error> {
    let global_excludes = config
        .excludes
        .clone() // for sanity purposes
        .unwrap_or(Vec::<String>::new());

    let mut sessions = Vec::<Session>::new();
    // Side-effects were needed
    config.search_roots.iter().for_each(|root| {
        let local_excludes = root.excludes.clone().unwrap_or_default();

        let _: Vec<_> = WalkDir::new(&root.path)
            .max_depth(root.depth.unwrap_or(config.depth))
            .into_iter()
            .filter_entry(|entry| {
                if is_excluded_from(&global_excludes, entry)
                    || is_excluded_from(&local_excludes, entry)
                {
                    return false;
                }

                // There was no other way to do it using walkdir
                if !config.search_subdirs {
                    add_if_repo(entry, &mut repo_names)
                } else {
                    add_if_repo(entry, &mut repo_names);
                    true
                }
            })
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().is_dir())
            .collect();
    });

    repo_names.iter().for_each(|repo| println!("{repo}"));
    Ok(repo_names)
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

fn file_name(entry: &DirEntry) -> String {
    entry.file_name().to_string_lossy().to_string()
}

//fn bdeduplicate
