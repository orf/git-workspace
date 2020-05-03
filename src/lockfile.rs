use crate::repository::Repository;
use anyhow::Context;
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

    pub fn read(&self) -> anyhow::Result<Vec<Repository>> {
        let config_data = fs::read_to_string(&self.path)
            .with_context(|| format!("Cannot read file {}", self.path.display()))?;
        let config: LockfileContents = toml::from_str(config_data.as_str())
            .with_context(|| "Error deserializing".to_string())?;
        Ok(config.repos)
    }

    pub fn write(&self, repositories: &[Repository]) -> anyhow::Result<()> {
        let mut sorted_repositories = repositories.to_owned();
        sorted_repositories.sort();

        let toml = toml::to_string(&LockfileContents {
            repos: sorted_repositories,
        })?;
        fs::write(&self.path, toml)
            .with_context(|| format!("Error writing lockfile to {}", self.path.display()))?;

        Ok(())
    }
}
