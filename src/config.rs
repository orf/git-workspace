use crate::providers::{GithubProvider, GitlabProvider, Provider};
use crate::repository::Repository;
use anyhow::Context;
use serde::{Deserialize, Serialize};
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

impl Config {
    pub fn new(files: Vec<PathBuf>) -> Config {
        Config { files }
    }

    // Find all config files in workspace
    fn find_config_files(workspace: &Path) -> anyhow::Result<Vec<PathBuf>> {
        let matcher = globset::GlobBuilder::new("workspace*.toml")
            .literal_separator(true)
            .build()?
            .compile_matcher();
        let entries = fs::read_dir(workspace)
            .with_context(|| format!("Cannot list directory {}", workspace.display()))?;
        let mut config_files: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .map(|n| n != "workspace-lock.toml" && matcher.is_match(n))
                    .unwrap_or(false)
            })
            .collect();
        config_files.sort();

        Ok(config_files)
    }

    pub fn from_workspace(workspace: &Path) -> anyhow::Result<Self> {
        let config_files =
            Self::find_config_files(workspace).context("Error loading config files")?;
        if config_files.is_empty() {
            anyhow::bail!("No configuration files found: Are you in the right workspace?")
        }
        Ok(Self::new(config_files))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    const WORKSPACE_FILE_CONTENT: &str = r#"[[provider]]
    provider = "github"
    name = "github-group"
    url = "https://api.github.com/graphql"
    path = "github"
    env_var = "GITHUB_TOKEN"
    skip_forks = false
    auth_http = true
    include = []
    exclude = []
    [[provider]]
    provider = "gitlab"
    name = "gitlab-group"
    url = "https://gitlab.com"
    path = "gitlab"
    env_var = "GITLAB_COM_TOKEN"
    auth_http = true
    include = []
    exclude = []"#;

    fn create_test_config(dir: &Path, filename: &str, content: &str) -> PathBuf {
        let config_path = dir.join(filename);
        let mut file = File::create(&config_path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        config_path
    }

    #[test]
    fn test_find_config_files() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path();

        // Create test config files
        create_test_config(dir_path, "workspace.toml", WORKSPACE_FILE_CONTENT);
        create_test_config(dir_path, "workspace-test.toml", WORKSPACE_FILE_CONTENT);
        create_test_config(dir_path, "workspace-lock.toml", "File should be ignored");
        create_test_config(dir_path, "other.toml", "File should be ignored");

        let config_files = Config::find_config_files(dir_path).unwrap();
        assert_eq!(config_files.len(), 2);
        assert!(config_files[0].ends_with("workspace-test.toml"));
        assert!(config_files[1].ends_with("workspace.toml"));
    }

    #[test]
    fn test_config_from_workspace() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path();

        // Test with no config files
        let result = Config::from_workspace(dir_path);
        assert!(result.is_err());

        // Test with config file
        create_test_config(dir_path, "workspace.toml", WORKSPACE_FILE_CONTENT);

        let config = Config::from_workspace(dir_path).unwrap();
        assert_eq!(config.files.len(), 1);
    }

    #[test]
    fn test_config_read() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path();

        create_test_config(dir_path, "workspace.toml", WORKSPACE_FILE_CONTENT);
        create_test_config(dir_path, "workspace-42.toml", WORKSPACE_FILE_CONTENT);

        let config = Config::from_workspace(dir_path).unwrap();
        let providers = config.read().unwrap();

        assert_eq!(providers.len(), 4);
        match &providers[0] {
            ProviderSource::Github(config) => assert_eq!(config.name, "github-group"),
            _ => panic!("Expected Github provider"),
        }
        match &providers[1] {
            ProviderSource::Gitlab(config) => assert_eq!(config.name, "gitlab-group"),
            _ => panic!("Expected Gitlab provider"),
        }
    }

    #[test]
    fn test_config_write() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("workspace.toml");

        let providers = vec![
            ProviderSource::Github(GithubProvider::default()),
            ProviderSource::Gitlab(GitlabProvider::default()),
        ];
        let config = Config::new(vec![config_path.clone()]);
        config.write(providers, &config_path).unwrap();

        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("github"));
        assert!(content.contains("gitlab"));
    }

    #[test]
    fn test_invalid_config_content() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path();

        // Create invalid config
        create_test_config(
            dir_path,
            "workspace.toml",
            r#"[[provider]]
            invalid = "content""#,
        );

        let config = Config::from_workspace(dir_path).unwrap();
        let result = config.read();
        assert!(result.is_err());
    }
}
