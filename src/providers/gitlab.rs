use crate::providers::Provider;
use crate::repository::Repository;
use console::style;
use failure::{Error, ResultExt};
use graphql_client::{GraphQLQuery, Response};
use serde::{Deserialize, Serialize};
use std::env;
use std::fmt;
use structopt::StructOpt;

// GraphQL queries we use to fetch user and group repositories.
// Right now, annoyingly, Gitlab has a bug around GraphQL pagination:
// https://gitlab.com/gitlab-org/gitlab/issues/33419
// So, we don't paginate at all in these queries. I'll fix this once
// the issue is closed.

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/providers/graphql/gitlab/schema.json",
    query_path = "src/providers/graphql/gitlab/projects.graphql",
    response_derives = "Debug"
)]
pub struct UserRepositories;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/providers/graphql/gitlab/schema.json",
    query_path = "src/providers/graphql/gitlab/projects.graphql",
    response_derives = "Debug"
)]
pub struct GroupRepositories;

static DEFAULT_GITLAB_URL: &str = "https://gitlab.com";

fn public_gitlab_url() -> String {
    DEFAULT_GITLAB_URL.to_string()
}

#[derive(Deserialize, Serialize, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[serde(untagged)]
#[serde(rename_all = "lowercase")]
#[derive(StructOpt)]
pub enum GitlabProvider {
    #[structopt(about = "Add a Gitlab user by name")]
    User {
        user: String,
        #[serde(default = "public_gitlab_url")]
        #[structopt(long = "url", default_value = DEFAULT_GITLAB_URL)]
        url: String,
        #[structopt(long = "path", default_value = "gitlab")]
        #[structopt(about = "Clone repositories to a specific base path")]
        path: String,
    },
    #[structopt(about = "Add a Gitlab group by name")]
    Group {
        group: String,
        #[serde(default = "public_gitlab_url")]
        #[structopt(long = "url", default_value = DEFAULT_GITLAB_URL)]
        url: String,
        #[structopt(long = "path", default_value = "gitlab/")]
        #[structopt(about = "Clone repositories to a specific base path")]
        path: String,
    },
}

impl fmt::Display for GitlabProvider {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self {
            GitlabProvider::User { user, url, path } => write!(
                f,
                "Gitlab user {} at {} in path {}",
                style(user).green(),
                style(url).green(),
                style(path).green()
            ),
            GitlabProvider::Group { group, url, path } => write!(
                f,
                "Gitlab group {} at {} in path {}",
                style(group).green(),
                style(url).green(),
                style(path).green()
            ),
        }
    }
}

impl GitlabProvider {
    /*
    Duplicating these two methods is madness, but I don't know enough about rust to make them generic.

    The issue is that while the structure of the types returned by the graphql query is identical,
    they are different types that live under a different namespace. And they don't share a
    common trait. I guess I could write my own trait for each node type, but that's a lot of
    effort and I've already been trying for several hours.
    */

    fn fetch_user_repositories(
        &self,
        path: &str,
        name: &str,
        url: &str,
    ) -> Result<Vec<Repository>, Error> {
        let github_token =
            env::var("GITLAB_TOKEN").context("Missing GITLAB_TOKEN environment variable")?;
        let mut repositories = vec![];
        let q = UserRepositories::build_query(user_repositories::Variables {
            name: name.to_string(),
            after: Some("".to_string()),
        });
        let res = ureq::post(format!("{}/api/graphql", url).as_str())
            .set("Authorization", format!("Bearer {}", github_token).as_str())
            .set("Content-Type", "application/json")
            .send_json(json!(&q));
        let json = res.into_json()?;
        let response_body: Response<user_repositories::ResponseData> =
            serde_json::from_value(json)?;
        let gitlab_repositories = response_body
            .data
            .expect("Missing data")
            .namespace
            .expect("Missing namespace")
            .projects
            .edges
            .expect("missing edges")
            .into_iter()
            // Some(T) -> T
            .filter_map(|x| x)
            // Extract the node, which is also Some(T)
            .filter_map(|x| x.node);

        for repo in gitlab_repositories {
            if repo.archived.unwrap() {
                continue;
            }
            let branch = repo.repository.and_then(|r| r.root_ref);
            repositories.push(Repository::new(
                format!("{}/{}", path, repo.full_path),
                repo.ssh_url_to_repo.expect("Unknown SSH URL"),
                branch,
                None,
            ));
        }
        Ok(repositories)
    }

    fn fetch_group_repositories(
        &self,
        path: &str,
        name: &str,
        url: &str,
    ) -> Result<Vec<Repository>, Error> {
        let github_token =
            env::var("GITLAB_TOKEN").context("Missing GITLAB_TOKEN environment variable")?;
        let mut repositories = vec![];
        let q = GroupRepositories::build_query(group_repositories::Variables {
            name: name.to_string(),
            after: Some("".to_string()),
        });
        let res = ureq::post(format!("{}/api/graphql", url).as_str())
            .set("Authorization", format!("Bearer {}", github_token).as_str())
            .set("Content-Type", "application/json")
            .send_json(json!(&q));
        let json = res.into_json()?;
        let response_body: Response<group_repositories::ResponseData> =
            serde_json::from_value(json)?;
        let gitlab_repositories = response_body
            .data
            .expect("Missing data")
            .group
            .expect("Missing namespace")
            .projects
            .edges
            .expect("missing edges")
            .into_iter()
            // Some(T) -> T
            .filter_map(|x| x)
            // Extract the node, which is also Some(T)
            .filter_map(|x| x.node);
        for repo in gitlab_repositories {
            if repo.archived.unwrap() {
                continue;
            }
            let branch = repo.repository.and_then(|r| r.root_ref);
            repositories.push(Repository::new(
                format!("{}/{}", path, repo.full_path),
                repo.ssh_url_to_repo.expect("Unknown SSH URL"),
                branch,
                None,
            ));
        }
        Ok(repositories)
    }
}

impl Provider for GitlabProvider {
    fn correctly_configured(&self) -> bool {
        let provider_url = match self {
            GitlabProvider::Group {
                group: _,
                url,
                path: _,
            } => url,
            GitlabProvider::User {
                user: _,
                url,
                path: _,
            } => url,
        };
        let token = env::var("GITLAB_TOKEN");
        if token.is_err() {
            println!(
                "{}",
                style("Error: GITLAB_TOKEN environment variable is not defined").red()
            );
            println!("Create a personal access token here:");
            println!("{}/profile/personal_access_tokens", provider_url);
            println!("Set a GITLAB_TOKEN environment variable with the value");
            return false;
        }
        true
    }
    fn fetch_repositories(&self) -> Result<Vec<Repository>, Error> {
        let repositories = match self {
            GitlabProvider::User { user, url, path } => {
                self.fetch_user_repositories(&path, user, url)?
            }
            GitlabProvider::Group { group, url, path } => {
                self.fetch_group_repositories(&path, group, url)?
            }
        };
        Ok(repositories)
    }
}
