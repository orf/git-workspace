use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(untagged)]
#[serde(rename_all = "lowercase")]
pub enum GitlabProvider {
    User { user: String, url: Option<String> },
    Group { group: String, url: Option<String> },
}
