use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum GithubProvider {
    User(String),
    Org(String),
}
