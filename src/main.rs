extern crate git2;
extern crate serde;

use crate::config::Config;
use crate::lockfile::Lockfile;
use crate::repository::Repository;
use std::path::Path;

mod config;
mod lockfile;
mod repository;

fn main() {
    let workspace = Path::new("workspace");

    let config = Config::new(workspace.join("workspace.toml"));
    config.read();
    let lockfile = config.lockfile();

    let repo = Repository::new(
        "test-repo".to_string(),
        "git@github.com:orf/dotfiles.git".to_string(),
        "master".to_string(),
    );
    let repo2 = Repository::new(
        "test-repo2".to_string(),
        "git@github.com:orf/dotfiles.git".to_string(),
        "master".to_string(),
    );
    println!("{}", repo.exists(workspace));
    println!("{:?}", repo.clone(workspace));
    lockfile.write(vec![repo, repo2]);
    println!("{:?}", lockfile.read());
}
