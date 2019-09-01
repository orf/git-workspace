use failure::Error;
use git2::Repository as Git2Repo;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command};

pub struct CloneError {}

// Eq, Ord and friends are needed to order the list of repositories
#[derive(Deserialize, Serialize, Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct Repository {
    path: String,
    url: String,
    branch: String,
}

impl Repository {
    pub fn new(path: String, url: String, branch: String) -> Repository {
        Repository { path, url, branch }
    }
    pub fn exists(&self, root: &PathBuf) -> bool {
        match Git2Repo::open(root.join(&self.path)) {
            Ok(_) => true,
            Err(_) => false,
        }
    }
    pub fn clone(&self, root: &PathBuf) -> Result<bool, Error> {
        let mut command = Command::new("git");

        let result = command
            .arg("clone")
            .arg("--recurse-submodules")
            .arg("--progress")
            .arg(&self.url)
            .arg(root.join(&self.path))
            .output()?;

        Ok(result.status.success())
    }
    pub fn full_path(&self, root: &Path) -> PathBuf {
        root.join(&self.path)
    }
}
