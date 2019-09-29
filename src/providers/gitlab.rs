use crate::providers::Provider;
use crate::repository::Repository;
use failure::Error;
use graphql_client::{GraphQLQuery, Response};
use serde::{Deserialize, Serialize};
use std::env;
use structopt::StructOpt;

static DEFAULT_GITLAB_URL: &'static str = "https://gitlab.com";

fn public_gitlab_url() -> String {
    DEFAULT_GITLAB_URL.to_string()
}

#[derive(Deserialize, Serialize, Debug)]
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
        #[structopt(long = "path", default_value = "gitlab")]
        #[structopt(about = "Clone repositories to a specific base path")]
        path: String,
    },
}

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

impl GitlabProvider {
    fn fetch_user_repositories(
        &self,
        path: &String,
        username: &String,
        url: &String,
    ) -> Result<Vec<Repository>, Error> {
        let github_token = env::var("GITLAB_TOKEN")?;
        let client = reqwest::Client::new();
        let mut repositories = vec![];
        let q = UserRepositories::build_query(user_repositories::Variables {
            user_name: username.to_string(),
        });
        let mut res = client
            .post(format!("{}/api/graphql", url).as_str())
            .bearer_auth(github_token.as_str())
            .json(&q)
            .send()?;
        let response_body: Response<user_repositories::ResponseData> = res.json()?;
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
}

impl Provider for GitlabProvider {
    fn fetch_repositories(&self) -> Result<Vec<Repository>, Error> {
        let repositories = match self {
            GitlabProvider::User { user, url, path } => {
                self.fetch_user_repositories(&path, user, url)?
            }
            GitlabProvider::Group { group, url, path } => vec![],
        };
        Ok(repositories)
    }
}
