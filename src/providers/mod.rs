mod github;
mod gitlab;
use crate::repository::Repository;
use failure::Error;
pub use github::GithubProvider;
pub use gitlab::GitlabProvider;

pub trait Provider {
    fn fetch_repositories(&self) -> Result<Vec<Repository>, Error>;
}
