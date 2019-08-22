use crate::lockfile::Lockfile;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub struct Config {
    path: PathBuf,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
enum GithubSource {
    User(String),
    Org(String),
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
enum GitlabSource {
    User(String),
    Group(String),
}

#[derive(Deserialize, Debug)]
#[serde(tag = "provider")]
#[serde(rename_all = "lowercase")]
enum RepositorySource {
    Gitlab(GitlabSource),
    Github(GithubSource),
}

impl Config {
    pub fn new(path: PathBuf) -> Config {
        Config { path }
    }
    pub fn lockfile(&self) -> Lockfile {
        Lockfile::new(self.path.parent().unwrap().join("workspace-lock.toml"))
    }
    pub fn read(&self) {
        let file_contents = fs::read_to_string(&self.path).unwrap();
        let contents: HashMap<String, RepositorySource> =
            toml::from_str(file_contents.as_str()).unwrap();
        println!("{:?}", contents);
    }
}
