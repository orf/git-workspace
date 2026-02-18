use crate::providers::{
    create_exclude_regex_set, create_include_regex_set, Provider, APP_USER_AGENT,
};
use crate::repository::Repository;
use anyhow::{bail, Context};
use console::style;
use graphql_client::{GraphQLQuery, Response};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use std::fmt;

// See https://github.com/graphql-rust/graphql-client/blob/master/graphql_client/tests/custom_scalars.rs#L6
type GitSSHRemote = String;
#[allow(clippy::upper_case_acronyms)]
type URI = String;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/providers/graphql/github/schema.graphql",
    query_path = "src/providers/graphql/github/projects.graphql",
    response_derives = "Debug"
)]
pub struct Repositories;

fn default_env_var() -> String {
    String::from("GITHUB_TOKEN")
}

static DEFAULT_GITHUB_URL: &str = "https://api.github.com/graphql";

fn public_github_url() -> String {
    DEFAULT_GITHUB_URL.to_string()
}

#[derive(Deserialize, Serialize, Default, Debug, Eq, Ord, PartialEq, PartialOrd, clap::Parser)]
#[serde(rename_all = "lowercase")]
#[command(about = "Add a Github user or organization by name")]
pub struct GithubProvider {
    /// The name of the user or organisation to add.
    pub name: String,
    #[arg(long = "path", default_value = "github")]
    /// Clone repositories to a specific base path
    path: String,
    #[arg(long = "env-name", short = 'e', default_value = "GITHUB_TOKEN")]
    #[serde(default = "default_env_var")]
    /// Environment variable containing the auth token
    env_var: String,

    #[arg(long = "skip-forks")]
    #[serde(default)]
    /// Don't clone forked repositories
    skip_forks: bool,

    #[arg(long = "include")]
    #[serde(default)]
    /// Only clone repositories that match these regular expressions. The repository name
    /// includes the user or organisation name.
    include: Vec<String>,

    #[arg(long = "auth-http")]
    #[serde(default)]
    /// Use HTTP authentication instead of SSH
    auth_http: bool,

    #[arg(long = "exclude")]
    #[serde(default)]
    /// Don't clone repositories that match these regular expressions. The repository name
    /// includes the user or organisation name.
    exclude: Vec<String>,

    #[serde(default = "public_github_url")]
    #[arg(long = "url", default_value = DEFAULT_GITHUB_URL)]
    /// Github instance URL, if using Github Enterprise this should be
    /// http(s)://HOSTNAME/api/graphql
    pub url: String,
}

impl fmt::Display for GithubProvider {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Github user/org {} in directory {}, using the token stored in {}",
            style(&self.name.to_lowercase()).green(),
            style(&self.path.to_lowercase()).green(),
            style(&self.env_var).green(),
        )
    }
}

impl GithubProvider {
    fn parse_repo(
        &self,
        path: &str,
        repo: &repositories::RepositoriesRepositoryOwnerRepositoriesNodes,
    ) -> Repository {
        let default_branch = repo
            .default_branch_ref
            .as_ref()
            .map(|branch| branch.name.clone());
        let upstream = repo.parent.as_ref().map(|parent| parent.ssh_url.clone());

        Repository::new(
            format!("{}/{}", path, repo.name_with_owner.clone()),
            if self.auth_http {
                repo.url.clone()
            } else {
                repo.ssh_url.clone()
            },
            default_branch,
            upstream,
        )
    }
}

impl Provider for GithubProvider {
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
            if self.url == public_github_url() {
                println!(
                    "Create a personal access token here: {}",
                    style("https://github.com/settings/tokens").green()
                );
            } else {
                println!(
                    "Create a personal access token in your {}.",
                    style("Github Enterprise server").green()
                );
            }

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
        let github_token = env::var(&self.env_var)
            .with_context(|| format!("Missing {} environment variable", self.env_var))?;

        let auth_header = match github_token.as_str() {
            "none" => "none".to_string(),
            token => {
                format!("Bearer {}", token)
            }
        };

        let mut repositories = vec![];

        let mut after = None;

        let include_regex_set = create_include_regex_set(&self.include)?;
        let exclude_regex_set = create_exclude_regex_set(&self.exclude)?;

        // include_forks needs to be None instead of true, as the graphql parameter has three
        // states: false - no forks, true - only forks, none - all repositories.
        let include_forks: Option<bool> = if self.skip_forks { Some(false) } else { None };

        let agent = ureq::AgentBuilder::new()
            .https_only(true)
            .user_agent(APP_USER_AGENT)
            .build();

        loop {
            let q = Repositories::build_query(repositories::Variables {
                login: self.name.to_lowercase(),
                include_forks,
                after,
            });
            let res = {
                let max_retries = 3;
                let mut last_err = None;
                let mut response = None;
                for attempt in 0..max_retries {
                    let result = agent
                        .post(&self.url)
                        .set("Authorization", &auth_header)
                        .send_json(json!(&q));
                    match result {
                        Ok(resp) => {
                            response = Some(resp);
                            break;
                        }
                        Err(e) => {
                            last_err = Some(e);
                            if attempt < max_retries - 1 {
                                std::thread::sleep(std::time::Duration::from_secs(1));
                            }
                        }
                    }
                }
                match response {
                    Some(resp) => resp,
                    None => {
                        let err = last_err.unwrap();
                        match err {
                            ureq::Error::Status(status, response) => match response.into_string() {
                                Ok(resp) => {
                                    bail!("Got status code {status}. Body: {resp}")
                                }
                                Err(e) => {
                                    bail!("Got status code {status}. Error reading body: {e}")
                                }
                            },
                            e => return Err(e.into()),
                        }
                    }
                }
            };

            let body = res.into_string()?;
            let response_data: Response<repositories::ResponseData> = serde_json::from_str(&body)?;

            if let Some(errors) = response_data.errors {
                let total_errors = errors.len();
                let combined_errors: Vec<_> = errors
                    .into_iter()
                    .map(|e| {
                        let mut message_str = e.message;
                        if let Some(path) = e.path {
                            let path_strings: Vec<String> =
                                path.iter().map(|p| p.to_string()).collect();
                            message_str.push_str(format!(" ({})", path_strings.join(".")).as_str());
                        }
                        message_str
                    })
                    .collect();
                let combined_message = combined_errors.join("\n");
                bail!(
                    "Received {} errors. Errors:\n{}",
                    total_errors,
                    combined_message
                );
            }

            let response_repositories = response_data
                .data
                .with_context(|| format!("Invalid response from GitHub: {}", body))?
                .repository_owner
                .with_context(|| format!("Invalid response from GitHub: {}", body))?
                .repositories;

            repositories.extend(
                response_repositories
                    .nodes
                    .unwrap()
                    .iter()
                    .map(|r| r.as_ref().unwrap())
                    .filter(|r| !r.is_archived)
                    .filter(|r| include_regex_set.is_match(&r.name_with_owner))
                    .filter(|r| !exclude_regex_set.is_match(&r.name_with_owner))
                    .map(|repo| self.parse_repo(&self.path, repo)),
            );

            if !response_repositories.page_info.has_next_page {
                break;
            }
            after = response_repositories.page_info.end_cursor;
        }

        Ok(repositories)
    }
}
