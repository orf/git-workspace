use crate::providers::Provider;
use crate::repository::Repository;
use failure::{Error, ResultExt};
use graphql_client::{GraphQLQuery, Response};
use serde::{Deserialize, Serialize};
use std::env;
use structopt::StructOpt;

#[derive(Deserialize, Serialize, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "lowercase")]
#[derive(StructOpt)]
#[structopt(about = "Add a Github user or organization by name")]
pub struct GithubProvider {
    pub name: String,
    #[structopt(long = "path", default_value = "github")]
    #[structopt(about = "Clone repositories to a specific base path")]
    path: String,
}

// See https://github.com/graphql-rust/graphql-client/blob/master/graphql_client/tests/custom_scalars.rs#L6
type GitSSHRemote = String;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/providers/graphql/github/schema.graphql",
    query_path = "src/providers/graphql/github/projects.graphql",
    response_derives = "Debug"
)]
pub struct Repositories;

impl GithubProvider {
    fn parse_repo(
        &self,
        path: &str,
        repo: &repositories::RepositoriesRepositoryOwnerRepositoriesNodes,
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
            format!("{}/{}", path, repo.name_with_owner.clone()),
            repo.ssh_url.clone(),
            default_branch,
            upstream,
        )
    }
}

impl Provider for GithubProvider {
    fn fetch_repositories(&self) -> Result<Vec<Repository>, Error> {
        let github_token =
            env::var("GITHUB_TOKEN").context("Missing GITHUB_TOKEN environment variable")?;
        let client = reqwest::Client::new();
        let mut repositories = vec![];

        let mut after = None;

        loop {
            let q = Repositories::build_query(repositories::Variables {
                login: self.name.clone(),
                after,
            });
            let mut res = client
                .post("https://api.github.com/graphql")
                .bearer_auth(github_token.as_str())
                .json(&q)
                .send()?;
            let response_body: Response<repositories::ResponseData> = res.json()?;
            let response_data: repositories::ResponseData =
                response_body.data.expect("missing response data");
            let response_repositories = response_data
                .repository_owner
                .expect("missing repository owner")
                .repositories;
            for repo in response_repositories
                .nodes
                .unwrap()
                .iter()
                .map(|r| r.as_ref().unwrap())
                .filter(|r| !r.is_archived)
            {
                repositories.push(self.parse_repo(&self.path, &repo))
            }

            if !response_repositories.page_info.has_next_page {
                break;
            }
            after = response_repositories.page_info.end_cursor;
        }

        Ok(repositories)
    }
}
