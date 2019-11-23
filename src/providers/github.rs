use crate::providers::Provider;
use crate::repository::Repository;
use console::style;
use failure::{Error, ResultExt};
use graphql_client::{GraphQLQuery, Response};
use serde::{Deserialize, Serialize};
use std::env;
use std::fmt;
use structopt::StructOpt;

// See https://github.com/graphql-rust/graphql-client/blob/master/graphql_client/tests/custom_scalars.rs#L6
type GitSSHRemote = String;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/providers/graphql/github/schema.graphql",
    query_path = "src/providers/graphql/github/projects.graphql",
    response_derives = "Debug"
)]
pub struct Repositories;

#[derive(Deserialize, Serialize, Debug, Eq, Ord, PartialEq, PartialOrd, StructOpt)]
#[serde(rename_all = "lowercase")]
#[structopt(about = "Add a Github user or organization by name")]
pub struct GithubProvider {
    pub name: String,
    #[structopt(long = "path", default_value = "github")]
    #[structopt(about = "Clone repositories to a specific base path")]
    path: String,
}

impl fmt::Display for GithubProvider {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Github user/org {} in directory {}",
            style(&self.name).green(),
            style(&self.path).green()
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
    fn correctly_configured(&self) -> bool {
        let token = env::var("GITHUB_TOKEN");
        if token.is_err() {
            println!(
                "{}",
                style("Error: GITHUB_TOKEN environment variable is not defined").red()
            );
            println!("Create a personal access token here:");
            println!("https://github.com/settings/tokens");
            println!("Set a GITHUB_TOKEN environment variable with the value");
            return false;
        }
        true
    }

    fn fetch_repositories(&self) -> Result<Vec<Repository>, Error> {
        let github_token =
            env::var("GITHUB_TOKEN").context("Missing GITHUB_TOKEN environment variable")?;
        let mut repositories = vec![];

        let mut after = None;

        loop {
            let q = Repositories::build_query(repositories::Variables {
                login: self.name.clone(),
                after,
            });
            let res = ureq::post("https://api.github.com/graphql")
                .set("Authorization", format!("Bearer {}", github_token).as_str())
                .send_json(json!(&q));
            let response_data: Response<repositories::ResponseData> =
                serde_json::from_value(res.into_json()?)?;
            let response_repositories = response_data
                .data
                .expect("Missing data")
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
