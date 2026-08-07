mod gitea;
mod github;
mod gitlab;

use crate::repository::Repository;
use anyhow::{bail, Context};
pub use gitea::GiteaProvider;
pub use github::GithubProvider;
pub use gitlab::GitlabProvider;
use std::fmt;

pub static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

/// The HTTP agent shared by every provider.
///
/// Roots come from the platform certificate store rather than the bundled Mozilla
/// set, so self-hosted instances sitting behind an internal CA keep working. This
/// also honours the `SSL_CERT_FILE` and `SSL_CERT_DIR` environment variables.
pub fn build_agent() -> anyhow::Result<ureq::Agent> {
    let native = rustls_native_certs::load_native_certs();
    if native.certs.is_empty() && !native.errors.is_empty() {
        bail!("Error loading native certificates: {:?}", native.errors);
    }
    let roots: Vec<_> = native
        .certs
        .iter()
        .map(|cert| ureq::tls::Certificate::from_der(cert.as_ref()).to_owned())
        .collect();

    Ok(ureq::Agent::config_builder()
        .https_only(true)
        .user_agent(APP_USER_AGENT)
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .root_certs(ureq::tls::RootCerts::new_with_certs(&roots))
                .build(),
        )
        .build()
        .new_agent())
}

pub trait Provider: fmt::Display {
    /// Returns true if the provider should work, otherwise prints an error and return false
    fn correctly_configured(&self) -> bool;
    fn fetch_repositories(&self) -> anyhow::Result<Vec<Repository>>;
}

pub fn create_exclude_regex_set(items: &Vec<String>) -> anyhow::Result<regex::RegexSet> {
    if items.is_empty() {
        Ok(regex::RegexSet::empty())
    } else {
        Ok(regex::RegexSet::new(items).context("Error parsing exclude regular expressions")?)
    }
}

pub fn create_include_regex_set(items: &Vec<String>) -> anyhow::Result<regex::RegexSet> {
    if items.is_empty() {
        let all = vec![".*"];
        Ok(regex::RegexSet::new(all).context("Error parsing include regular expressions")?)
    } else {
        Ok(regex::RegexSet::new(items).context("Error parsing include regular expressions")?)
    }
}
