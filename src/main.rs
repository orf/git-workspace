#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate toml;
extern crate git2;

use git2::Repository;

#[derive(Deserialize)]
struct Config {
    ip: String,
    port: Option<u16>,
    keys: Keys,
}

#[derive(Deserialize)]
struct Keys {
    github: String,
    travis: Option<String>,
}

fn main() {
    let config: Config = toml::from_str(r#"
        ip = '127.0.0.1'

        [keys]
        github = 'xxxxxxxxxxxxxxxxx'
    "#).unwrap();

    assert_eq!(config.ip, "127.0.0.1");
    assert_eq!(config.port, None);
    assert_eq!(config.keys.github, "xxxxxxxxxxxxxxxxx");
    assert_eq!(config.keys.travis.as_ref().unwrap(), "yyyyyyyyyyyyyyyyy");

    let repo = match Repository::init("/tmp/empty-rep!o") {
        Ok(repo) => repo,
        Err(e) => panic!("failed to init: {}", e),
    };
}
