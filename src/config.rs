use crate::providers::{GithubProvider, GitlabProvider, Provider};
use crate::repository::Repository;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Serialize, Debug)]
struct ConfigContents {
    #[serde(rename = "provider", default)]
    providers: Vec<ProviderSource>,
}

pub struct Config {
    files: Vec<PathBuf>,
}

pub fn all_config_files(workspace: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let matcher = globset::GlobBuilder::new("workspace*.toml")
        .literal_separator(true)
        .build()?
        .compile_matcher();
    let entries: Vec<OsString> = fs::read_dir(workspace)
        .with_context(|| format!("Cannot list directory {}", workspace.display()))?
        .map(|res| res.map(|e| e.file_name()))
        .collect::<Result<Vec<_>, std::io::Error>>()?;
    let mut entries_that_exist: Vec<PathBuf> = entries
        .into_iter()
        .filter(|p| p != "workspace-lock.toml" && matcher.is_match(p))
        .map(|p| workspace.join(p))
        .collect();
    entries_that_exist.sort();

    Ok(entries_that_exist)
}

impl Config {
    pub fn new(files: Vec<PathBuf>) -> Config {
        Config { files }
    }

    pub fn read(&self) -> anyhow::Result<Vec<ProviderSource>> {
        let mut all_providers = vec![];

        for path in &self.files {
            if !path.exists() {
                continue;
            }
            let file_contents = fs::read_to_string(path)
                .with_context(|| format!("Cannot read file {}", path.display()))?;
            let contents: ConfigContents = toml::from_str(file_contents.as_str())
                .with_context(|| format!("Error parsing TOML in file {}", path.display()))?;
            all_providers.extend(contents.providers);
        }
        Ok(all_providers)
    }
    pub fn write(&self, providers: Vec<ProviderSource>, config_path: &Path) -> anyhow::Result<()> {
        let toml = toml::to_string(&ConfigContents { providers })?;
        fs::write(config_path, toml)
            .with_context(|| format!("Error writing to file {}", config_path.display()))?;
        Ok(())
    }
}

#[derive(Deserialize, Serialize, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[serde(tag = "provider")]
#[serde(rename_all = "lowercase")]
#[derive(clap::Subcommand)]
pub enum ProviderSource {
    Gitlab(GitlabProvider),
    Github(GithubProvider),
}

impl ProviderSource {
    pub fn provider(&self) -> &dyn Provider {
        match self {
            Self::Gitlab(config) => config,
            Self::Github(config) => config,
        }
    }

    pub fn correctly_configured(&self) -> bool {
        self.provider().correctly_configured()
    }

    pub fn fetch_repositories(&self) -> anyhow::Result<Vec<Repository>> {
        self.provider().fetch_repositories()
    }
}

impl fmt::Display for ProviderSource {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.provider())
    }
}
