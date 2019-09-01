use crate::providers::Provider;
use crate::repository::Repository;
use failure::Error;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum GithubProvider {
    User(String),
    Org(String),
}

impl Provider for GithubProvider {
    fn fetch_repositories(&self) -> Result<Vec<Repository>, Error> {
        Ok(vec![])
    }
}
