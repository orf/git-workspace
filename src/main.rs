extern crate git2;
extern crate serde;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate structopt;

use crate::config::Config;
use crate::lockfile::Lockfile;
use crate::repository::Repository;
use failure::Error;
use std::path::{Path, PathBuf};
use structopt::StructOpt;

mod config;
mod lockfile;
mod repository;

#[derive(StructOpt)]
#[structopt(name = "git-workspace", about = "Manage your git repositories")]
struct Opt {
    #[structopt(short = "w", long = "workspace", parse(from_os_str))]
    workspace: PathBuf,
    #[structopt(subcommand)]
    command: Command,
}

#[derive(StructOpt)]
enum Command {
    #[structopt(name = "update")]
    Update {},
    #[structopt(name = "lock")]
    Lock {},
    #[structopt(name = "list")]
    List {},
}

fn main() -> Result<(), Error> {
    let opt = Opt::from_args();
    let workspace_path = opt.workspace.canonicalize()?;

    match opt.command {
        Command::List {} => list(&workspace_path)?,
        Command::Update {} => update(&workspace_path)?,
        Command::Lock {} => lock(&workspace_path)?,
    };
    Ok(())
}

fn update(workspace: &PathBuf) -> Result<(), Error> {
    Ok(())
}

fn lock(workspace: &PathBuf) -> Result<(), Error> {
    let repo = Repository::new(
        "test-repo".to_string(),
        "git@github.com:orf/dotfiles.git".to_string(),
        "master".to_string(),
    );
    let repo2 = Repository::new(
        "test-repo".to_string(),
        "git@github.com:orf/dotfiles.git".to_string(),
        "master".to_string(),
    );
    let lockfile = Lockfile::new(workspace.join("workspace-lock.toml"));
    lockfile.write(vec![repo, repo2])?;
    Ok(())
}

fn list(workspace: &PathBuf) -> Result<(), Error> {
    let lockfile = Lockfile::new(workspace.join("workspace-lock.toml"));
    let repositories = lockfile.read()?;
    for repo in repositories {
        println!("{}", repo.full_path(workspace).to_string_lossy());
    }
    Ok(())
}

/*

    let workspace = Path::new("workspace");

    let config = Config::new(workspace.join("workspace.toml"));

    println!("Config read: {:?}", config.read());
    println!("Lock read: {:?}", lockfile.read());

    println!("Exists: {}", repo.exists(workspace));
    println!("Clone: {:?}", repo.clone(workspace));

    println!("Lock read 2: {:?}", lockfile.read());
*/
