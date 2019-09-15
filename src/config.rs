use failure::Error;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::providers::{GithubProvider, GitlabProvider, Provider};
use crate::repository::Repository;

pub struct Config {
    path: PathBuf,
}

impl Config {
    pub fn new(path: PathBuf) -> Config {
        Config { path }
    }
    pub fn read(&self) -> Result<HashMap<String, ProviderSource>, Error> {
        let file_contents = fs::read_to_string(&self.path)?;
        let contents: HashMap<String, ProviderSource> = toml::from_str(file_contents.as_str())?;
        Ok(contents)
    }
}

#[derive(Deserialize, Debug)]
#[serde(tag = "provider")]
#[serde(rename_all = "lowercase")]
pub enum ProviderSource {
    Gitlab(GitlabProvider),
    Github(GithubProvider),
}

impl ProviderSource {
    fn provider(&self) -> &dyn Provider {
        match self {
            Self::Gitlab(config) => config,
            Self::Github(config) => config,
        }
    }

    pub fn fetch_repositories(&self, root: &String) -> Result<Vec<Repository>, Error> {
        Ok(self.provider().fetch_repositories(root)?)
    }
}
