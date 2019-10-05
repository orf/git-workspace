#[macro_use]
extern crate failure;
extern crate console;
#[cfg(unix)]
extern crate expanduser;
extern crate fs_extra;
extern crate graphql_client;
extern crate indicatif;
extern crate reqwest;
extern crate serde;
extern crate structopt;
extern crate walkdir;

use crate::config::{Config, ProviderSource};
use crate::lockfile::Lockfile;
use crate::progress::ProgressManager;
use crate::repository::Repository;
use console::style;
use failure::{Error, ResultExt};
use indicatif::{MultiProgress, ParallelProgressIterator, ProgressBar};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::collections::HashSet;
use std::path::PathBuf;
use std::thread::JoinHandle;
use std::{process, thread};
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

fn main() {
    let args = Args::from_args();
    if let Err(e) = handle_main(args) {
        eprintln!("{}", style("There was an internal error!").red());
        for cause in e.iter_chain() {
            eprintln!("{}", style(cause).red());
        }
        process::exit(1);
    }
}

fn handle_main(args: Args) -> Result<(), Error> {
    let path_str = args
        .workspace
        .canonicalize()
        .context(format!("{} does not exist", args.workspace.display()))?;
    let workspace_path;
    #[cfg(not(unix))]
    {
        workspace_path = PathBuf::from(path_str);
    }
    #[cfg(unix)]
    {
        workspace_path = expanduser::expanduser(path_str.to_string_lossy())
            .context("Error expanding git workspace path")?;
    }

    match args.command {
        Command::List {} => list(&workspace_path)?,
        Command::Update { threads } => {
            lock(&workspace_path)?;
            update(&workspace_path, threads)?
        }
        Command::Fetch { threads } => fetch(&workspace_path, threads)?,
        Command::Add(provider) => add_provider_to_config(&workspace_path, provider)?,
    };
    Ok(())
}

fn add_provider_to_config(
    workspace: &PathBuf,
    provider_source: ProviderSource,
) -> Result<(), Error> {
    let config = Config::new(workspace.join("workspace.toml"));
    let mut sources = config.read().context("Error reading config file")?;
    if sources.iter().any(|s| s == &provider_source) {
        println!("Entry already exists, skipping");
    } else {
        sources.push(provider_source);
        config.write(sources).context("Error writing config file")?;
        println!("Added entry to workspace.toml");
    }
    Ok(())
}

fn map_repositories<F>(repositories: &[Repository], threads: usize, f: F) -> Result<(), Error>
where
    F: Fn(&Repository, &ProgressBar) -> Result<(), Error> + std::marker::Sync,
{
    let progress = MultiProgress::new();
    let manager = ProgressManager::new(&progress, threads);
    let total_bar = progress.add(manager.create_total_bar(repositories.len() as u64));

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .context("Error creating the thread pool")?;

    let waiting_thread: JoinHandle<std::result::Result<(), Error>> = thread::spawn(move || {
        progress.join_and_clear()?;
        Ok(())
    });

    // pool.install means that `.par_iter()` will use the thread pool we've built above.
    let errors: Vec<(&Repository, Error)> = pool.install(|| {
        repositories
            .par_iter()
            .progress_with(total_bar)
            .map(|repo| {
                let progress_bar = manager.get_bar();
                let result = match f(repo, &progress_bar) {
                    Ok(_) => Ok(()),
                    Err(e) => Err((repo, e)),
                };
                manager.put_bar(progress_bar);
                result
            })
            .filter_map(Result::err)
            .collect()
    });
    manager.signal_done();
    waiting_thread.join();

    if !errors.is_empty() {
        eprintln!("{} repositories failed to clone:", errors.len());
        for (repo, error) in errors {
            eprint!("{}: {}", repo.name(), style(error).red())
        }
    }

    Ok(())
}

fn update(workspace: &PathBuf, threads: usize) -> Result<(), Error> {
    let lockfile = Lockfile::new(workspace.join("workspace-lock.toml"));
    let repositories = lockfile.read().context("Error reading lockfile")?;

    let repos_to_clone: Vec<Repository> = repositories
        .iter()
        .filter(|r| !r.exists(workspace))
        .cloned()
        .collect();

    println!("Cloning {} repositories", repos_to_clone.len(),);

    map_repositories(&repos_to_clone, threads, |r, progress_bar| {
        r.clone(&workspace, &progress_bar)?;
        r.set_upstream(&workspaxe)?;
        Ok(())
    })?;
    archive_repositories(workspace, repositories).context("Error archiving repository")?;

    Ok(())
}

fn fetch(workspace: &PathBuf, threads: usize) -> Result<(), Error> {
    let lockfile = Lockfile::new(workspace.join("workspace-lock.toml"));
    let repositories = lockfile.read()?;

    let repos_to_fetch: Vec<Repository> = repositories
        .iter()
        .filter(|r| r.exists(workspace))
        .cloned()
        .collect();

    println!("Fetching {} repositories", repos_to_fetch.len(),);

    map_repositories(&repos_to_fetch, threads, |r, progress_bar| {
        r.fetch(&workspace, &progress_bar)
    })?;

    Ok(())
}

fn archive_repositories(workspace: &PathBuf, repositories: Vec<Repository>) -> Result<(), Error> {
    // The logic here is as follows:
    // 1. Iterate through all directories. If it's a "safe" directory (one that contains a project
    //    in our lockfile), we skip it entirely.
    // 2. If the directory is not, and contains a `.git` directory, then we mark it for archival and
    //    skip processing.
    // This assumes nobody deletes a .git directory in one of their projects.
    let archive_directory = workspace.join(".archive");

    let mut safe_paths: HashSet<PathBuf> = repositories
        .iter()
        .map(|r| r.full_path(workspace))
        .collect();
    safe_paths.insert(archive_directory.clone());

    let mut to_archive = Vec::new();
    let mut it = WalkDir::new(workspace).into_iter();

    // I couldn't work out how to use `filter_entry` here, so we just roll our own loop.

    loop {
        let entry = match it.next() {
            None => break,
            Some(Err(err)) => bail!("Error iterating through directory: {}", err),
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
        fs_extra::dir::create(&archive_directory, false).context(format!(
            "Error creating archive directory {}",
            archive_directory.display()
        ))?;
    }

    for from_dir in to_archive.iter() {
        let relative_dir = from_dir.strip_prefix(workspace)?;
        let to_dir = archive_directory.join(relative_dir);
        println!("Archiving {}", relative_dir.display());
        if to_dir.exists() {
            fs_extra::dir::remove(&to_dir)
                .context(format!("Error removing directory {}", to_dir.display()))?;
        }
        fs_extra::dir::create_all(&to_dir, false)
            .context(format!("Error creating directory {}", to_dir.display()))?;
        fs_extra::dir::move_dir(&from_dir, &to_dir.parent().unwrap(), &options).context(
            format!(
                "Error moving directory {} to {}",
                from_dir.display(),
                to_dir.display()
            ),
        )?;
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
