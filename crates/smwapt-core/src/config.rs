use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Paths {
    pub root: PathBuf,
    pub state_dir: PathBuf,
    pub sources_list: PathBuf,
    pub cache_dir: PathBuf,
    pub downloads_dir: PathBuf,
    pub repo_dir: PathBuf,
}

impl Paths {
    pub fn new(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref().to_path_buf();
        let state_dir = root.join(".smwapt");
        Self {
            sources_list: state_dir.join("sources.list"),
            cache_dir: state_dir.join("cache"),
            downloads_dir: state_dir.join("downloads"),
            repo_dir: state_dir.join("repo"),
            state_dir,
            root,
        }
    }
}
