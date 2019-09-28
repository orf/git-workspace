use crate::providers::Provider;
use crate::repository::Repository;
use failure::Error;
use graphql_client::{GraphQLQuery, Response};
use serde::Deserialize;
use std::env;

fn public_gitlab_url() -> String {
    "https://gitlab.com".to_string()
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
#[serde(rename_all = "lowercase")]
pub enum GitlabProvider {
    User {
        user: String,
        #[serde(default = "public_gitlab_url")]
        url: String,
    },
    Group {
        group: String,
        #[serde(default = "public_gitlab_url")]
        url: String,
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
        root: &String,
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
                format!("{}/{}", root, repo.full_path),
                repo.ssh_url_to_repo.expect("Unknown SSH URL"),
                branch,
                None,
            ));
        }
        Ok(repositories)
    }
}

impl Provider for GitlabProvider {
    fn fetch_repositories(&self, root: &String) -> Result<Vec<Repository>, Error> {
        let repositories = match self {
            GitlabProvider::User { user, url } => self.fetch_user_repositories(root, user, url)?,
            GitlabProvider::Group { group, url } => vec![],
        };
        Ok(repositories)
    }
}
