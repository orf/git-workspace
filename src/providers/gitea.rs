use crate::providers::{
    create_exclude_regex_set, create_include_regex_set, Provider, APP_USER_AGENT,
};
use crate::repository::Repository;
use anyhow::Context;
use console::style;
use serde::{Deserialize, Serialize};
use std::env;
use std::fmt;

#[derive(Deserialize, Debug)]
struct GiteaRepository {
    full_name: String,
    clone_url: String,
    ssh_url: String,
    default_branch: String,
    archived: bool,
    fork: bool,
}

fn default_env_var() -> String {
    String::from("GITEA_TOKEN")
}

static DEFAULT_GITEA_URL: &str = "https://gitea.com";

fn public_gitea_url() -> String {
    DEFAULT_GITEA_URL.to_string()
}

#[derive(Deserialize, Serialize, Debug, Eq, Ord, PartialEq, PartialOrd, clap::Parser)]
#[serde(rename_all = "lowercase")]
#[command(about = "Add a Gitea user or organization by name")]
pub struct GiteaProvider {
    /// The name of the user or organisation to add
    pub name: String,

    #[arg(long = "path", default_value = "gitea")]
    /// Clone repos to a specific path
    path: String,

    #[arg(long = "env-name", short = 'e', default_value = "GITEA_TOKEN")]
    #[serde(default = "default_env_var")]
    /// Environment variable containing the auth token
    env_var: String,

    #[arg(long = "skip-forks")]
    #[serde(default)]
    /// Don't clone forked repositories
    skip_forks: bool,

    #[arg(long = "include")]
    #[serde(default)]
    /// Only clone repositories that match these regular expressions
    include: Vec<String>,

    #[arg(long = "auth-http")]
    #[serde(default)]
    /// Use HTTP authentication instead of SSH
    auth_http: bool,

    #[arg(long = "exclude")]
    #[serde(default)]
    /// Don't clone repositories that match these regular expressions
    exclude: Vec<String>,

    #[arg(long = "url", default_value = DEFAULT_GITEA_URL)]
    #[serde(default = "public_gitea_url")]
    /// Gitea instance URL
    pub url: String,
}

impl fmt::Display for GiteaProvider {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Gitea user/org {} at {} in directory {}, using the token stored in {}",
            style(&self.name.to_lowercase()).green(),
            style(&self.url).green(),
            style(&self.path).green(),
            style(&self.env_var).green(),
        )
    }
}

impl Provider for GiteaProvider {
    fn correctly_configured(&self) -> bool {
        let token = env::var(&self.env_var);
        if token.is_err() {
            println!(
                "{}",
                style(format!(
                    "Error: {} environment variable is not defined",
                    self.env_var
                ))
                .red()
            );
            println!("Create an access token in your Gitea Settings -> Applications");
            println!(
                "Then set a {} environment variable with the value",
                style(&self.env_var).green()
            );
            return false;
        }
        if self.name.ends_with('/') {
            println!(
                "{}",
                style("Error: Ensure that names do not end in forward slashes").red()
            );
            println!("You specified: {}", self.name);
            return false;
        }
        true
    }

    fn fetch_repositories(&self) -> anyhow::Result<Vec<Repository>> {
        let gitea_token = env::var(&self.env_var)
            .with_context(|| format!("Missing {} environment variable", self.env_var))?;

        let include_regex_set = create_include_regex_set(&self.include)?;
        let exclude_regex_set = create_exclude_regex_set(&self.exclude)?;

        let agent = ureq::AgentBuilder::new()
            .https_only(true)
            .user_agent(APP_USER_AGENT)
            .build();

        let mut page = 1;
        let mut repositories = Vec::new();

        loop {
            let url = format!(
                "{}/api/v1/users/{}/repos?page={}&limit=50",
                self.url, self.name, page
            );

            let response = agent
                .get(&url)
                .set("Authorization", &format!("token {}", gitea_token))
                .call()?;

            let repos: Vec<GiteaRepository> = response.into_json()?;
            if repos.is_empty() {
                break;
            }

            repositories.extend(
                repos
                    .into_iter()
                    .filter(|r| !r.archived)
                    .filter(|r| !self.skip_forks || !r.fork)
                    .filter(|r| include_regex_set.is_match(&r.full_name))
                    .filter(|r| !exclude_regex_set.is_match(&r.full_name))
                    .map(|r| {
                        Repository::new(
                            format!("{}/{}", self.path, r.full_name),
                            if self.auth_http {
                                r.clone_url
                            } else {
                                r.ssh_url
                            },
                            Some(r.default_branch),
                            None,
                        )
                    }),
            );

            page += 1;
        }

        Ok(repositories)
    }
}
