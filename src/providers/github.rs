use crate::providers::Provider;
use crate::repository::Repository;
use failure::Error;
use reqwest;
use serde::Deserialize;
use reqwest::header;

fn default_as_false() -> bool {
    false
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase", untagged)]
pub enum GithubProvider {
    User {
        user: String,
        #[serde(default = "default_as_false")]
        ignore_forks: bool,
    },
    Org {
        org: String,
        #[serde(default = "default_as_false")]
        ignore_forks: bool,
    },
}

#[derive(Deserialize, Debug)]
struct GithubRepo {
    id: u32,
    name: String,
    fork: bool,
    ssh_url: String,
    default_branch: String,
    full_name: String,
}

// Needed to avoid self-referencial structures
#[derive(Deserialize, Debug)]
struct GithubForkRepo {
    parent: GithubRepo,
}

impl Provider for GithubProvider {
    fn fetch_repositories(&self) -> Result<Vec<Repository>, Error> {
        let mut headers = header::HeaderMap::new();
        headers.insert(header::AUTHORIZATION, header::HeaderValue::from_static("token a9e25d33a505bdf4f559226a9d42d95311cf2e47"));
        headers.insert(header::ACCEPT, header::HeaderValue::from_static("application/vnd.github.v3+json"));
        let client = reqwest::Client::builder()
            .default_headers(headers).build()?;

        let mut repositories = vec![];
        let mut gh_resp =
            client.get("https://api.github.com/users/orf/repos?per_page=100")
                .send()?;
        let next_page = gh_resp.headers().get(header::LINK).unwrap();
        println!("{:?}", next_page);

        let gh_repositories: Vec<GithubRepo> = gh_resp.json()?;
        for repo in gh_repositories {
            let mut upstream_url = None;
            if repo.fork {
                let upstream_api_url =
                    format!("https://api.github.com/repos/{name}", name = repo.full_name);
                let upstream_api_response: GithubForkRepo =
                    client.get(upstream_api_url.as_str()).send()?.json()?;
                upstream_url = Some(upstream_api_response.parent.ssh_url);
            }
            repositories.push(
                Repository::new(
                    repo.full_name,
                    repo.ssh_url,
                    repo.default_branch,
                    upstream_url,
                )
            )
        }

        Ok(repositories)
    }
}
