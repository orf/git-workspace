extern crate console;
extern crate failure;
extern crate fs_extra;
extern crate git2;
extern crate graphql_client;
extern crate indicatif;
extern crate reqwest;
extern crate serde;
extern crate structopt;
extern crate walkdir;

use crate::config::{Config, ProviderSource};
use crate::lockfile::Lockfile;
use crate::progress::ProgressManager;
use failure::Error;
use indicatif::{MultiProgress, ParallelProgressIterator, ProgressBar, ProgressStyle};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::thread;
use structopt::StructOpt;
use walkdir::WalkDir;

mod config;
mod lockfile;
mod progress;
mod providers;
mod repository;

#[derive(StructOpt)]
#[structopt(name = "git-workspace", author, about)]
struct Args {
    #[structopt(
        short = "w",
        long = "workspace",
        parse(from_os_str),
        env = "GIT_WORKSPACE"
    )]
    workspace: PathBuf,
    #[structopt(subcommand)]
    command: Command,
}

#[derive(StructOpt)]
enum Command {
    Update {
        #[structopt(short = "t", long = "threads", default_value = "4")]
        threads: usize,
    },
    Fetch {
        #[structopt(short = "t", long = "threads", default_value = "4")]
        threads: usize,
    },
    List {},
    Add(ProviderSource),
}

#[paw::main]
fn main(args: Args) -> Result<(), Error> {
    let workspace_path = args.workspace.canonicalize()?;

    match args.command {
        Command::List {} => list(&workspace_path)?,
        Command::Update { threads } => {
            lock(&workspace_path)?;
            update(&workspace_path, threads, false)?
        }
        Command::Fetch { threads } => update(&workspace_path, threads, true)?,
        Command::Add(provider) => add_provider_to_config(&workspace_path, provider)?,
    };
    Ok(())
}

fn add_provider_to_config(
    workspace: &PathBuf,
    provider_source: ProviderSource,
) -> Result<(), Error> {
    let config = Config::new(workspace.join("workspace.toml"));
    let mut sources = config.read()?;
    sources.push(provider_source);
    config.write(sources)?;
    Ok(())
}

fn update(workspace: &PathBuf, threads: usize, fetch: bool) -> Result<(), Error> {
    let lockfile = Lockfile::new(workspace.join("workspace-lock.toml"));
    let repositories = lockfile.read()?;

    let progress = MultiProgress::new();
    let manager = ProgressManager::new(&progress, threads);
    let total_bar = progress.add(manager.create_total_bar(repositories.len() as u64));

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()?;

    let waiting_thread = thread::spawn(move || {
        progress.join_and_clear();
    });

    // pool.install means that `.par_iter()` will use the thread pool we've built above.
    pool.install(|| {
        repositories
            .par_iter()
            .progress_with(total_bar)
            .for_each(|repo| {
                let bar = manager.get_bar();
                bar.set_message("starting");
                if fetch && repo.exists(workspace) {
                    repo.fetch(workspace, &bar);
                } else if !fetch && !repo.exists(workspace) {
                    repo.clone(workspace, &bar);
                }
                manager.put_bar(bar);
            })
    });
    manager.signal_done();

    waiting_thread.join();

    let archive_directory = workspace.join(".archive");

    let mut safe_paths: HashSet<PathBuf> = repositories
        .iter()
        .map(|r| r.full_path(workspace))
        .collect();
    safe_paths.insert(archive_directory.clone());

    let mut to_archive = Vec::new();

    // I couldn't work out how to use `filter_entry` here, so we just roll our own loop.
    // The logic here is as follows:
    // 1. Iterate through all directories. If it's a "safe" directory (one that contains a project
    //    in our lockfile), we skip it entirely.
    // 2. If the directory is not, and contains a `.git` directory, then we mark it for archival and
    //    skip processing.
    // This assumes nobody deletes a .git directory in one of their projects.
    let mut it = WalkDir::new(workspace).into_iter();

    loop {
        let entry = match it.next() {
            None => break,
            Some(Err(err)) => panic!("ERROR: {}", err),
            Some(Ok(entry)) => entry,
        };
        if safe_paths.contains(entry.path()) {
            it.skip_current_dir();
            continue;
        }
        if entry.path().join(".git").is_dir() {
            to_archive.push(entry.path().to_path_buf());
            it.skip_current_dir();
            continue;
        }
    }

    let options = fs_extra::dir::CopyOptions::new();

    if !archive_directory.exists() && !to_archive.is_empty() {
        fs_extra::dir::create(&archive_directory, false);
    }

    for from_dir in to_archive.iter() {
        let relative_dir = from_dir.strip_prefix(workspace)?;
        let to_dir = archive_directory.join(relative_dir);
        println!("Archiving {}", relative_dir.display());
        if to_dir.exists() {
            fs_extra::dir::remove(&to_dir);
        }
        fs_extra::dir::create_all(&to_dir, false);
        fs_extra::dir::move_dir(&from_dir, &to_dir.parent().unwrap(), &options)?;
    }
    Ok(())
}

fn lock(workspace: &PathBuf) -> Result<(), Error> {
    let config = Config::new(workspace.join("workspace.toml"));
    let sources = config.read()?;
    let mut all_repositories = vec![];
    for source in sources.iter() {
        all_repositories.extend(source.fetch_repositories()?);
    }
    // We may have duplicated repositories here. Make sure they are unique based on the full path.
    all_repositories.sort();
    all_repositories.dedup();
    let lockfile = Lockfile::new(workspace.join("workspace-lock.toml"));
    lockfile.write(&all_repositories)?;
    println!(
        "Found {} repositories from {} users or groups",
        all_repositories.len(),
        sources.len()
    );
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
