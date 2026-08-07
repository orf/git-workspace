use crate::providers::{build_agent, create_exclude_regex_set, create_include_regex_set, Provider};
use crate::repository::Repository;
use anyhow::{anyhow, bail, Context};
use clap::ValueEnum;
use console::style;
use graphql_client::{GraphQLQuery, Response};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use std::fmt;
use std::process::Command;
use url::Url;

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

/// Where the API token comes from.
///
/// Configs written before this existed have no `auth_type` key, and fall back to
/// `EnvVar` — which is what every such config meant.
#[derive(
    Deserialize, Serialize, Default, Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd, ValueEnum,
)]
#[serde(rename_all = "kebab-case")]
pub enum GithubAuthType {
    /// Read the token from the environment variable named by `--env-name`.
    #[default]
    EnvVar,
    /// Ask the Github CLI for a token by running `gh auth token`.
    GhCli,
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
    /// Environment variable containing the auth token. Unused when --auth-type
    /// is gh-cli.
    env_var: String,

    #[arg(long = "auth-type", value_enum, default_value_t = GithubAuthType::EnvVar)]
    #[serde(default)]
    /// Where to read the auth token from
    auth_type: GithubAuthType,

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
        let auth = match self.auth_type {
            GithubAuthType::EnvVar => {
                format!("the token stored in {}", style(&self.env_var).green())
            }
            GithubAuthType::GhCli => format!("the {} CLI", style("github").green()),
        };
        write!(
            f,
            "Github user/org {} in directory {}, using {}",
            style(&self.name.to_lowercase()).green(),
            style(&self.path.to_lowercase()).green(),
            auth,
        )
    }
}

impl GithubProvider {
    /// The token used to authenticate against the API, from whichever source
    /// `auth_type` selects.
    fn fetch_token(&self) -> anyhow::Result<String> {
        match self.auth_type {
            GithubAuthType::EnvVar => env::var(&self.env_var)
                .with_context(|| format!("Missing {} environment variable", self.env_var)),
            GithubAuthType::GhCli => self.fetch_gh_cli_token(),
        }
    }

    /// Shell out to `gh auth token`, which reads whatever credentials the user has
    /// already set up with `gh auth login`.
    fn fetch_gh_cli_token(&self) -> anyhow::Result<String> {
        let mut command = Command::new("gh");
        command.args(["auth", "token"]);
        // Without an explicit hostname `gh` assumes github.com, which is wrong for
        // Github Enterprise.
        if let Some(hostname) = self.hostname() {
            command.args(["--hostname", &hostname]);
        }

        let output = command.output().with_context(|| {
            "Error running `gh`. Is the Github CLI installed and on your PATH? \
             See https://cli.github.com"
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stderr = stderr.trim();
            if stderr.is_empty() {
                bail!("`gh auth token` exited with {}", output.status);
            }
            bail!("`gh auth token` failed: {stderr}");
        }

        let token = String::from_utf8(output.stdout)
            .context("`gh auth token` returned a token that is not valid UTF-8")?
            .trim()
            .to_string();
        if token.is_empty() {
            bail!("`gh auth token` returned an empty token. Run `gh auth login` to authenticate.");
        }
        Ok(token)
    }

    /// The host to pass to `gh --hostname`, derived from the API URL.
    fn hostname(&self) -> Option<String> {
        // The public API lives on api.github.com, but `gh` knows it as github.com.
        if self.url == DEFAULT_GITHUB_URL {
            return Some("github.com".to_string());
        }
        Url::parse(&self.url).ok()?.host_str().map(str::to_string)
    }

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
        match self.auth_type {
            GithubAuthType::EnvVar if env::var(&self.env_var).is_err() => {
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
            GithubAuthType::GhCli => {
                if let Err(e) = self.fetch_gh_cli_token() {
                    println!("{}", style(format!("Error: {e:#}")).red());
                    println!(
                        "Authenticate with {} and try again.",
                        style("gh auth login").green()
                    );
                    return false;
                }
            }
            GithubAuthType::EnvVar => {}
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
        let github_token = self.fetch_token()?;

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

        let agent = build_agent()?;

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
                    // Handle the status ourselves so that a failure response can be reported
                    // along with its body, which explains *why* GitHub rejected the query.
                    let result = agent
                        .post(&self.url)
                        .config()
                        .http_status_as_error(false)
                        .build()
                        .header("Authorization", &auth_header)
                        .send_json(json!(&q));
                    match result {
                        Ok(resp) if resp.status().is_success() => {
                            response = Some(resp);
                            break;
                        }
                        Ok(mut resp) => {
                            let status = resp.status().as_u16();
                            last_err = Some(match resp.body_mut().read_to_string() {
                                Ok(body) => anyhow!("Got status code {status}. Body: {body}"),
                                Err(e) => {
                                    anyhow!("Got status code {status}. Error reading body: {e}")
                                }
                            });
                        }
                        Err(e) => last_err = Some(e.into()),
                    }
                    if attempt < max_retries - 1 {
                        std::thread::sleep(std::time::Duration::from_secs(1));
                    }
                }
                match response {
                    Some(resp) => resp,
                    None => return Err(last_err.unwrap()),
                }
            };

            let body = res.into_body().read_to_string()?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use toml::toml;

    /// Configs written before `auth_type` existed must keep working untouched.
    #[test]
    fn test_deserialize_without_auth_type() {
        let provider: GithubProvider = toml! {
            name = "github-group"
            path = "github"
            env_var = "MY_TOKEN"
        }
        .try_into()
        .unwrap();
        assert_eq!(provider.auth_type, GithubAuthType::EnvVar);
        assert_eq!(provider.env_var, "MY_TOKEN");
    }

    #[test]
    fn test_deserialize_without_env_var_or_auth_type() {
        let provider: GithubProvider = toml! {
            name = "github-group"
            path = "github"
        }
        .try_into()
        .unwrap();
        assert_eq!(provider.auth_type, GithubAuthType::EnvVar);
        assert_eq!(provider.env_var, default_env_var());
    }

    #[test]
    fn test_deserialize_gh_cli_auth_type() {
        let provider: GithubProvider = toml! {
            name = "github-group"
            path = "github"
            auth_type = "gh-cli"
        }
        .try_into()
        .unwrap();
        assert_eq!(provider.auth_type, GithubAuthType::GhCli);
    }

    #[test]
    fn test_auth_type_round_trips_through_toml() {
        let provider: GithubProvider = toml! {
            name = "github-group"
            path = "github"
            auth_type = "gh-cli"
        }
        .try_into()
        .unwrap();
        let serialized = toml::to_string(&provider).unwrap();
        assert!(serialized.contains(r#"auth_type = "gh-cli""#));
        assert_eq!(
            toml::from_str::<GithubProvider>(&serialized).unwrap(),
            provider
        );
    }

    /// Old command lines must keep parsing, and default to the environment variable.
    #[test]
    fn test_cli_defaults_to_env_var() {
        let provider = GithubProvider::parse_from(["github", "some-org"]);
        assert_eq!(provider.auth_type, GithubAuthType::EnvVar);
        assert_eq!(provider.env_var, default_env_var());
    }

    #[test]
    fn test_cli_env_name_still_works() {
        let provider = GithubProvider::parse_from(["github", "some-org", "-e", "MY_TOKEN"]);
        assert_eq!(provider.auth_type, GithubAuthType::EnvVar);
        assert_eq!(provider.env_var, "MY_TOKEN");
    }

    #[test]
    fn test_cli_auth_type_gh_cli() {
        let provider = GithubProvider::parse_from(["github", "some-org", "--auth-type", "gh-cli"]);
        assert_eq!(provider.auth_type, GithubAuthType::GhCli);
    }

    #[test]
    fn test_cli_rejects_unknown_auth_type() {
        assert!(
            GithubProvider::try_parse_from(["github", "some-org", "--auth-type", "nope"]).is_err()
        );
    }

    #[test]
    fn test_hostname() {
        let hostname =
            |url: &str| GithubProvider::parse_from(["github", "some-org", "--url", url]).hostname();
        assert_eq!(hostname(DEFAULT_GITHUB_URL).as_deref(), Some("github.com"));
        assert_eq!(
            hostname("https://ghe.example.com/api/graphql").as_deref(),
            Some("ghe.example.com")
        );
        assert_eq!(
            hostname("https://ghe.example.com:8443/api/graphql").as_deref(),
            Some("ghe.example.com")
        );
        assert_eq!(
            hostname("https://user:pass@ghe.example.com/api/graphql").as_deref(),
            Some("ghe.example.com")
        );
        // An unparseable URL means no --hostname, so `gh` falls back to its default.
        assert_eq!(hostname("not a url"), None);
    }
}
