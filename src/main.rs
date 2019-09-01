extern crate failure;
extern crate git2;
extern crate serde;
extern crate structopt;

use crate::config::Config;
use crate::lockfile::Lockfile;
use crate::repository::Repository;
use failure::Error;
use std::path::PathBuf;
use structopt::StructOpt;

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
    let config = Config::new(workspace.join("workspace.toml"));
    let sources = config.read()?;
    for source in sources.values() {
        source.fetch_repositories()?;
        //providers::fetch_repositories(&source);
    }
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




    println!("Lock read: {:?}", lockfile.read());

    println!("Exists: {}", repo.exists(workspace));
    println!("Clone: {:?}", repo.clone(workspace));

    println!("Lock read 2: {:?}", lockfile.read());
*/
