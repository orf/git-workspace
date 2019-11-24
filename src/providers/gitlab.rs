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
pub struct Repositories;

struct ProjectNode {
    archived: bool,
    full_path: String,
    ssh_url: String,
    root_ref: Option<String>,
}

impl From<repositories::RepositoriesGroupProjectsEdgesNode> for ProjectNode {
    fn from(item: repositories::RepositoriesGroupProjectsEdgesNode) -> Self {
        Self {
            archived: item.archived.unwrap(),
            root_ref: item.repository.and_then(|r| r.root_ref),
            ssh_url: item.ssh_url_to_repo.expect("Unknown SSH URL"),
            full_path: item.full_path,
        }
    }
}

impl From<repositories::RepositoriesNamespaceProjectsEdgesNode> for ProjectNode {
    fn from(item: repositories::RepositoriesNamespaceProjectsEdgesNode) -> Self {
        Self {
            archived: item.archived.unwrap(),
            root_ref: item.repository.and_then(|r| r.root_ref),
            ssh_url: item.ssh_url_to_repo.expect("Unknown SSH URL"),
            full_path: item.full_path,
        }
    }
}

static DEFAULT_GITLAB_URL: &str = "https://gitlab.com";

fn public_gitlab_url() -> String {
    DEFAULT_GITLAB_URL.to_string()
}

#[derive(Deserialize, Serialize, Debug, Eq, Ord, PartialEq, PartialOrd, StructOpt)]
#[serde(rename_all = "lowercase")]
#[structopt(about = "Add a Gitlab user or group by name")]
pub struct GitlabProvider {
    pub name: String,
    #[serde(default = "public_gitlab_url")]
    #[structopt(long = "url", default_value = DEFAULT_GITLAB_URL)]
    pub url: String,
    #[structopt(long = "path", default_value = "gitlab")]
    #[structopt(about = "Clone repositories to a specific base path")]
    path: String,
    #[structopt(long = "env-name", short = "e", default_value = "GITLAB_TOKEN")]
    #[structopt(about = "Use the token stored in this environment variable for authentication")]
    env_var: String,
}

impl fmt::Display for GitlabProvider {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Gitlab user/group {} at {} in directory {}, using the token stored in {}",
            style(&self.name.to_lowercase()).green(),
            style(&self.url).green(),
            style(&self.path).green(),
            style(&self.env_var).green(),
        )
    }
}

impl Provider for GitlabProvider {
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
            println!("Create a personal access token here:");
            println!("{}/profile/personal_access_tokens", self.url);
            println!(
                "Set an environment variable called {} with the value",
                self.env_var
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
    fn fetch_repositories(&self) -> Result<Vec<Repository>, Error> {
        let gitlab_token = env::var(&self.env_var)
            .context(format!("Missing {} environment variable", self.env_var))?;
        let mut repositories = vec![];
        let mut after = Some("".to_string());
        loop {
            let q = Repositories::build_query(repositories::Variables {
                name: self.name.to_string().to_lowercase(),
                after,
            });
            let res = ureq::post(format!("{}/api/graphql", self.url).as_str())
                .set("Authorization", format!("Bearer {}", gitlab_token).as_str())
                .set("Content-Type", "application/json")
                .send_json(json!(&q));
            let json = res.into_json()?;
            let response_body: Response<repositories::ResponseData> = serde_json::from_value(json)?;
            let data = response_body.data.expect("Missing data");

            let temp_repositories: Vec<ProjectNode>;

            // This is annoying but I'm still not sure how to unify it.
            if data.group.is_some() {
                let group_data = data.group.expect("Missing group").projects;
                temp_repositories = group_data
                    .edges
                    .expect("missing edges")
                    .into_iter()
                    // Some(T) -> T
                    .filter_map(|x| x)
                    // Extract the node, which is also Some(T)
                    .filter_map(|x| x.node)
                    .map(ProjectNode::from)
                    .collect();
                after = group_data.page_info.end_cursor;
            } else {
                let namespace_data = data.namespace.expect("Missing group").projects;
                temp_repositories = namespace_data
                    .edges
                    .expect("missing edges")
                    .into_iter()
                    // Some(T) -> T
                    .filter_map(|x| x)
                    // Extract the node, which is also Some(T)
                    .filter_map(|x| x.node)
                    .map(ProjectNode::from)
                    .collect();
                after = namespace_data.page_info.end_cursor;
            }

            for repo in temp_repositories {
                if repo.archived {
                    continue;
                }
                repositories.push(Repository::new(
                    format!("{}/{}", self.path, repo.full_path),
                    repo.ssh_url,
                    repo.root_ref,
                    None,
                ));
            }

            if after.is_none() {
                break;
            }
        }
        Ok(repositories)
    }
}
