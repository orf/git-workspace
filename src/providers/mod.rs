mod github;
mod gitlab;

use crate::repository::Repository;
use anyhow::anyhow;
pub use github::GithubProvider;
pub use gitlab::GitlabProvider;
use serde_json::Value;
use std::fmt;

use ureq::Response;

pub trait Provider: fmt::Display {
    /// Returns true if the provider should work, otherwise prints an error and return false
    fn correctly_configured(&self) -> bool;
    fn fetch_repositories(&self) -> anyhow::Result<Vec<Repository>>;
}

pub fn resp_to_json(response: Response) -> anyhow::Result<Value> {
    if !response.ok() {
        let error_text = if let Some(syn_error) = response.synthetic_error() {
            format!("{}", syn_error)
        } else {
            format!("Status code {}", response.status())
        };
        return Err(anyhow!("Error: {}", error_text));
    }

    Ok(response.into_json()?)
}
