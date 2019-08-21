extern crate git2;

#[macro_use]
extern crate lazy_static;

use crate::repository::Repository;
use std::path::Path;

mod repository;

fn main() {
    let workspace = Path::new("workspace");
    let repo = Repository::new(
        "test-repo".to_string(),
        "git@github.com:orf/dotfiles.git".to_string(),
        "master".to_string(),
    );
    println!("{}", repo.exists(workspace));
    println!("{:?}", repo.clone(workspace));
}
