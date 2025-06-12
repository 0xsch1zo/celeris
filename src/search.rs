use crate::config::Config;
use std::fs;

pub fn search(config: &Config) {
    let global_exclude = &config.exclude_directories;
    config.search_roots.iter().for_each(|root| {})
}
