use crate::repos::Repo;
use std::path::PathBuf;

pub struct SessionProperties {
    pub name: String,
    pub path: PathBuf,
}

impl From<Repo> for SessionProperties {
    fn from(value: Repo) -> Self {
        Self {
            name: value.name,
            path: value.path,
        }
    }
}
