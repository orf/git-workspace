use crate::providers::Provider;
use crate::repository::Repository;
use failure::Error;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(untagged)]
#[serde(rename_all = "lowercase")]
pub enum GitlabProvider {
    User { user: String, url: Option<String> },
    Group { group: String, url: Option<String> },
}

impl Provider for GitlabProvider {
    fn fetch_repositories(&self, root: &String) -> Result<Vec<Repository>, Error> {
        Ok(vec![])
    }
}
