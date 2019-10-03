use failure::{Error, ResultExt};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use structopt::StructOpt;

use crate::providers::{GithubProvider, GitlabProvider, Provider};
use crate::repository::Repository;

#[derive(Deserialize, Serialize, Debug)]
struct ConfigContents {
    #[serde(rename = "provider", default)]
    providers: Vec<ProviderSource>,
}

pub struct Config {
    path: PathBuf,
}

impl Config {
    pub fn new(path: PathBuf) -> Config {
        Config { path }
    }
    pub fn read(&self) -> Result<Vec<ProviderSource>, Error> {
        if !self.path.exists() {
            fs::File::create(&self.path).context(format!(
                "Cannot create config at path {}",
                self.path.display()
            ))?;
        }
        let file_contents = fs::read_to_string(&self.path)
            .context(format!("Cannot read file {}", self.path.display()))?;
        let contents: ConfigContents = toml::from_str(file_contents.as_str()).context(format!(
            "Error parsing TOML in file {}",
            self.path.display()
        ))?;
        Ok(contents.providers)
    }
    pub fn write(&self, providers: Vec<ProviderSource>) -> Result<(), Error> {
        let toml = toml::to_string(&ConfigContents { providers })?;
        fs::write(&self.path, toml)
            .context(format!("Error writing to file {}", self.path.display()))?;
        Ok(())
    }
}

#[derive(Deserialize, Serialize, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[serde(tag = "provider")]
#[serde(rename_all = "lowercase")]
#[derive(StructOpt)]
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

    pub fn fetch_repositories(&self) -> Result<Vec<Repository>, Error> {
        Ok(self.provider().fetch_repositories()?)
    }
}
