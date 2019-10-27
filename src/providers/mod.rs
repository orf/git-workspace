mod github;
mod gitlab;

use crate::repository::Repository;
use failure::Error;
pub use github::GithubProvider;
pub use gitlab::GitlabProvider;
use std::fmt;

pub trait Provider: fmt::Display {
    /// Returns true if the provider should work, otherwise prints an error and return false
    fn correctly_configured(&self) -> bool;
    fn fetch_repositories(&self) -> Result<Vec<Repository>, Error>;
}
