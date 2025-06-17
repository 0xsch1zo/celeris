use crate::config::Config;
use crate::sessions::{Session, Sessions};
use std::io;
use std::path::Path;
use walkdir::{DirEntry, WalkDir};

pub fn search(config: &Config) -> Result<Vec<Session>, io::Error> {
    let global_excludes = config
        .excludes
        .clone() // for sanity purposes
        .unwrap_or(Vec::<String>::new());

    let mut sessions = Sessions::new();
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
                    sessions.push_if_repo(entry)
                } else {
                    sessions.push_if_repo(entry);
                    true
                }
            })
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().is_dir())
            .collect();
    });

    let smth = sessions.get();
    smth.iter().for_each(|sesh| println!("{sesh:?}"));
    Ok(smth)
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
