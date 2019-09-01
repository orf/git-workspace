use failure::Error;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::providers::{GithubProvider, GitlabProvider};

pub struct Config {
    path: PathBuf,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "provider")]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Gitlab(GitlabProvider),
    Github(GithubProvider),
}

impl Config {
    pub fn new(path: PathBuf) -> Config {
        Config { path }
    }
    pub fn read(&self) -> Result<HashMap<String, Provider>, Error> {
        let file_contents = fs::read_to_string(&self.path)?;
        let contents: HashMap<String, Provider> = toml::from_str(file_contents.as_str())?;
        Ok(contents)
    }
}
