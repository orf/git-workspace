use std::collections::HashMap;

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum GitlabIdentity {
    User { user: String },
    Group { group: String },
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum GithubIdentity {
    User { user: String },
    Organization { org: String },
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
#[serde(tag = "provider")]
pub enum Workspace {
    Github {
        #[serde(flatten)]
        identity: GithubIdentity,
    },
    Gitlab {
        #[serde(default = "default_gitlab_url")]
        url: String,
        #[serde(flatten)]
        identity: GitlabIdentity,
    },
}

fn default_gitlab_url() -> String {
    "https://gitlab.com/".to_string()
}

pub fn get_config() -> HashMap<String, Workspace> {
    let config_str = include_str!("../../workspace/workspace.toml");
    return toml::from_str(config_str).unwrap();
}
