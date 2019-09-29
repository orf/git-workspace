mod github;
mod gitlab;
use crate::repository::Repository;
use failure::Error;
pub use github::GithubProvider;
pub use gitlab::GitlabProvider;

pub trait Provider {
    fn fetch_repositories(&self) -> Result<Vec<Repository>, Error>;
}

//trait Provider<T> {
//    fn fetch_repositories(&self, source: &T) -> Result<Vec<Repository>, Error>;
//}
//
//pub fn fetch_repositories(source: &Provider) -> Result<Vec<Repository>, Error> {
//    // This is probably a totally stupid way of doing this.
//    match source {
//        Provider::Gitlab(config) => config.fetch_repositories(config),
//        Provider::Github(config) => config.fetch_repositories(config),
//    }
//}

// https://play.rust-lang.org/?version=stable&mode=debug&edition=2018&gist=656651475ff9dedf65b828dc97d9edc3
