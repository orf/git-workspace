use crate::providers::Provider;
use crate::repository::Repository;
use failure::Error;
use graphql_client::{GraphQLQuery, Response};
use serde::Deserialize;
use std::env;

fn default_as_false() -> bool {
    false
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase", untagged)]
pub enum GithubProvider {
    User { user: String },
    Org { org: String },
}

// See https://github.com/graphql-rust/graphql-client/blob/master/graphql_client/tests/custom_scalars.rs#L6
type GitSSHRemote = String;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/providers/graphql/github/schema.graphql",
    query_path = "src/providers/graphql/github/projects.graphql",
    response_derives = "Debug"
)]
pub struct UserRepositories;

impl GithubProvider {
    fn parse_repo(
        &self,
        root: &String,
        repo: &user_repositories::UserRepositoriesViewerRepositoriesNodes,
    ) -> Repository {
        let default_branch = repo
            .default_branch_ref
            .as_ref()
            .and_then(|branch| Some(branch.name.clone()));
        let upstream = repo
            .parent
            .as_ref()
            .and_then(|parent| Some(parent.ssh_url.clone()));

        Repository::new(
            format!("{}/{}", root, repo.name_with_owner.clone()),
            repo.ssh_url.clone(),
            default_branch,
            upstream,
        )
    }
}

impl Provider for GithubProvider {
    fn fetch_repositories(&self, root: &String) -> Result<Vec<Repository>, Error> {
        let github_token = env::var("GITHUB_TOKEN")?;
        let client = reqwest::Client::new();
        let mut repositories = vec![];

        let mut after = None;

        loop {
            let q = UserRepositories::build_query(user_repositories::Variables { after });
            let mut res = client
                .post("https://api.github.com/graphql")
                .bearer_auth(github_token.as_str())
                .json(&q)
                .send()?;
            let response_body: Response<user_repositories::ResponseData> = res.json()?;
            let response_data: user_repositories::ResponseData =
                response_body.data.expect("missing response data");
            for repo in response_data.viewer.repositories.nodes.unwrap().iter() {
                repositories.push(self.parse_repo(root, repo.as_ref().unwrap()))
            }

            if !response_data.viewer.repositories.page_info.has_next_page {
                break;
            }
            after = response_data.viewer.repositories.page_info.end_cursor;
        }

        Ok(repositories)
    }
}
