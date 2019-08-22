use crate::repository::Repository;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub struct Lockfile {
    path: PathBuf,
}

#[derive(Deserialize, Serialize, Debug)]
struct LockfileContents {
    #[serde(rename = "repo")]
    repos: Vec<Repository>,
}

impl Lockfile {
    pub fn new(path: PathBuf) -> Lockfile {
        Lockfile { path }
    }

    pub fn read(&self) -> Vec<Repository> {
        let config_data = fs::read_to_string(&self.path).unwrap();
        let config: LockfileContents = toml::from_str(config_data.as_str()).unwrap();
        config.repos
    }

    pub fn write(&self, repositories: Vec<Repository>) {
        let toml = toml::to_string(&LockfileContents {
            repos: repositories,
        })
        .unwrap();
        fs::write(&self.path, toml);
    }
}
