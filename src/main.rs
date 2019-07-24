#[macro_use]
extern crate serde;
extern crate toml;

mod config;
mod github;
mod gitlab;
mod lockfile;

fn main() {
    let config = config::get_config();
    for (name, workspace) in config {
        println!("Name: {}, provider: {:?}", name, workspace)
    }

    gitlab::get_projects();
}
