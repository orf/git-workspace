extern crate failure;
extern crate git2;
extern crate reqwest;
extern crate serde;
extern crate structopt;
extern crate rayon;

use crate::config::Config;
use crate::lockfile::Lockfile;
use crate::repository::Repository;
use failure::Error;
use std::path::PathBuf;
use structopt::StructOpt;
use rayon::prelude::*;

mod config;
mod lockfile;
mod providers;
mod repository;

#[derive(StructOpt)]
#[structopt(name = "git-workspace", author, about)]
struct Args {
    #[structopt(short = "w", long = "workspace", parse(from_os_str))]
    workspace: PathBuf,
    #[structopt(subcommand)]
    command: Command,
}

#[derive(StructOpt)]
enum Command {
    Update {},
    Lock {},
    List {},
}

#[paw::main]
fn main(args: Args) -> Result<(), Error> {
    let workspace_path = args.workspace.canonicalize()?;

    match args.command {
        Command::List {} => list(&workspace_path)?,
        Command::Update {} => update(&workspace_path)?,
        Command::Lock {} => lock(&workspace_path)?,
    };
    Ok(())
}

fn update(workspace: &PathBuf) -> Result<(), Error> {
    let lockfile = Lockfile::new(workspace.join("workspace-lock.toml"));
    let repositories = lockfile.read()?;
    repositories.par_iter().for_each(|repo| {
        println!("{}", repo.full_path(workspace).to_string_lossy());
        if !repo.exists(workspace) {
            repo.clone(workspace);
        }
    });
    Ok(())
}

fn lock(workspace: &PathBuf) -> Result<(), Error> {
    let config = Config::new(workspace.join("workspace.toml"));
    let sources = config.read()?;
    let mut all_repositories = vec![];
    for source in sources.values() {
        all_repositories.extend(source.fetch_repositories()?);
    }
    let lockfile = Lockfile::new(workspace.join("workspace-lock.toml"));
    lockfile.write(all_repositories)?;
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
