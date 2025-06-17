use std::collections::HashSet;
use std::hash::Hash;
use walkdir::DirEntry;

pub fn is_unique<T>(iter: T) -> bool
where
    T: IntoIterator,
    T::Item: Eq + Hash,
{
    let mut uniq = HashSet::new();
    iter.into_iter().all(move |x| uniq.insert(x))
}

pub fn file_name(entry: &DirEntry) -> String {
    entry.file_name().to_string_lossy().to_string()
}
