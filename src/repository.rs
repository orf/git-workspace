use failure::Error;
use git2::Repository as Git2Repo;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

pub struct CloneError {}

#[derive(Deserialize, Serialize, Debug, Clone, Eq)]
pub struct Repository {
    path: String,
    url: String,
    branch: String,
}

impl Ord for Repository {
    fn cmp(&self, other: &Self) -> Ordering {
        self.path.cmp(&other.path)
    }
}

impl PartialOrd for Repository {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Repository {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
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
