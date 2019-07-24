use std::collections::HashMap;

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum GitlabIdentity {
    User { user: String },
    Namespace { namespace: String },
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
        url: Option<String>,
        #[serde(flatten)]
        identity: GitlabIdentity,
    },
}

pub fn get_config() -> HashMap<String, Workspace> {
    let config_str = include_str!("../workspace/workspace.toml");
    return toml::from_str(config_str).unwrap();
}
