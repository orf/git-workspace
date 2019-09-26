extern crate failure;
extern crate git2;
extern crate graphql_client;
extern crate reqwest;
extern crate serde;
extern crate structopt;
extern crate walkdir;

use crate::config::Config;
use crate::lockfile::Lockfile;
use crate::progress::ProgressSender;
use failure::Error;
use rayon::prelude::*;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::thread;
use structopt::StructOpt;
use walkdir::{DirEntry, WalkDir};

mod config;
mod lockfile;
mod progress;
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

    let (tx, rx) = channel();

    let sender = ProgressSender::new(tx);

    repositories
        .par_iter()
        .for_each_with(sender, |sender, repo| {
            sender.notify(&"hello there".to_string());
            if !repo.exists(workspace) {
                println!("{}", repo.full_path(workspace).to_string_lossy());
                //repo.clone(workspace);
            }
        });

    let directory_roots: Vec<DirEntry> = WalkDir::new(workspace)
        .into_iter()
        .filter_entry(|e| !e.path().join(".git").is_dir())
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_dir() && e.path() != workspace)
        .collect();
    println!("{:?}", directory_roots);
    Ok(())
}

fn lock(workspace: &PathBuf) -> Result<(), Error> {
    let config = Config::new(workspace.join("workspace.toml"));
    let sources = config.read()?;
    let mut all_repositories = vec![];
    for (name, source) in sources.iter() {
        all_repositories.extend(source.fetch_repositories(name)?);
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
