use crate::lockfile::Lockfile;
use failure::Error;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub struct Config {
    path: PathBuf,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum GithubSource {
    User(String),
    Org(String),
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum GitlabSource {
    User(String),
    Group(String),
}

#[derive(Deserialize, Debug)]
#[serde(tag = "provider")]
#[serde(rename_all = "lowercase")]
pub enum RepositorySource {
    Gitlab(GitlabSource),
    Github(GithubSource),
}

impl Config {
    pub fn new(path: PathBuf) -> Config {
        Config { path }
    }
    pub fn read(&self) -> Result<HashMap<String, RepositorySource>, Error> {
        let file_contents = fs::read_to_string(&self.path)?;
        let contents: HashMap<String, RepositorySource> = toml::from_str(file_contents.as_str())?;
        Ok(contents)
    }
}
