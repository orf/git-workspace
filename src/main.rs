#[macro_use]
extern crate failure;
extern crate atomic_counter;
extern crate console;
#[cfg(unix)]
extern crate expanduser;
extern crate fs_extra;
extern crate graphql_client;
extern crate indicatif;
extern crate serde;
extern crate ureq;
#[macro_use]
extern crate serde_json;
extern crate structopt;
extern crate walkdir;

use crate::config::{Config, ProviderSource};
use crate::lockfile::Lockfile;
use crate::repository::Repository;
use atomic_counter::{AtomicCounter, RelaxedCounter};
use console::style;
use failure::{Error, ResultExt};
use indicatif::{MultiProgress, ParallelProgressIterator, ProgressBar, ProgressStyle};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::collections::HashSet;
use std::path::PathBuf;
use std::thread::JoinHandle;
use std::{process, thread};
use structopt::StructOpt;
use walkdir::WalkDir;

mod config;
mod lockfile;
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
    List {
        #[structopt(long = "full")]
        full: bool,
    },
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
    let workspace_path;
    #[cfg(not(unix))]
    {
        workspace_path = PathBuf::from(args.workspace);
    }
    #[cfg(unix)]
    {
        workspace_path = expanduser::expanduser(args.workspace.to_string_lossy())
            .context("Error expanding git workspace path")?;
    }

    let path_str = (if workspace_path.exists() {
        &workspace_path
    } else {
        fs_extra::dir::create_all(&workspace_path, false).context(format!(
            "Error creating workspace directory {}",
            &workspace_path.display()
        ))?;
        println!("Created {} as it did not exist", &workspace_path.display());

        &workspace_path
    })
    .canonicalize()
    .context(format!(
        "Error canonicalizing workspace path {}",
        &workspace_path.display()
    ))?;

    match args.command {
        Command::List { full } => list(&workspace_path, full)?,
        Command::Update { threads } => {
            lock(&path_str)?;
            update(&path_str, threads)?
        }
        Command::Fetch { threads } => fetch(&path_str, threads)?,
        Command::Add(provider) => add_provider_to_config(&path_str, provider)?,
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

use std::sync::Arc;

fn map_repositories<F>(repositories: &[Repository], threads: usize, f: F) -> Result<(), Error>
where
    F: Fn(&Repository, &ProgressBar) -> Result<(), Error> + std::marker::Sync,
{
    let progress = Arc::new(MultiProgress::new());
    let total_bar = progress.add(ProgressBar::new(repositories.len() as u64));
    total_bar.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {percent}% [{wide_bar:.cyan/blue}] {pos}/{len} (ETA: {eta_precise})")
            .progress_chars("#>-"),
    );

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .context("Error creating the thread pool")?;

    let is_attended = console::user_attended();
    let total_repositories = repositories.len();
    let counter = RelaxedCounter::new(1);

    let progress_wait = progress.clone();

    let waiting_thread: JoinHandle<std::result::Result<(), Error>> = thread::spawn(move || {
        progress_wait.join()?;
        Ok(())
    });
    // pool.install means that `.par_iter()` will use the thread pool we've built above.
    let errors: Vec<(&Repository, Error)> = pool.install(|| {
        repositories
            .par_iter()
            .progress_with(total_bar)
            .map(|repo| {
                let progress_bar = progress.add(ProgressBar::new_spinner());
                progress_bar.set_message("waiting...");
                progress_bar.enable_steady_tick(100);
                let idx = counter.inc();
                if !is_attended {
                    println!("[{}/{}] Starting {}", idx, total_repositories, repo.name());
                }
                let result = match f(repo, &progress_bar) {
                    Ok(_) => Ok(()),
                    Err(e) => Err((repo, e)),
                };
                if !is_attended {
                    println!("[{}/{}] Finished {}", idx, total_repositories, repo.name());
                }
                progress_bar.finish_and_clear();
                result
            })
            .filter_map(Result::err)
            .collect()
    });

    waiting_thread.join();

    if !errors.is_empty() {
        eprintln!("{} repositories failed:", errors.len());
        for (repo, error) in errors {
            eprintln!("{}:", repo.name());
            for cause in error.iter_chain() {
                eprintln!(" - {}", style(cause).red());
            }
        }
    }

    Ok(())
}

fn update(workspace: &PathBuf, threads: usize) -> Result<(), Error> {
    let lockfile = Lockfile::new(workspace.join("workspace-lock.toml"));
    let repositories = lockfile.read().context("Error reading lockfile")?;

    println!("Updating {} repositories", repositories.len());

    map_repositories(&repositories, threads, |r, progress_bar| {
        if !r.exists(workspace) {
            r.clone(&workspace, &progress_bar)?;
        }
        r.set_upstream(&workspace)?;
        Ok(())
    })?;
    archive_repositories(workspace, repositories).context("Error archiving repositories")?;

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
    let archive_directory = if cfg!(windows) {
        workspace.join("_archive")
    } else {
        workspace.join(".archive")
    };

    let mut repository_paths: HashSet<PathBuf> = repositories
        .iter()
        .filter(|r| r.exists(workspace))
        .map(|r| r.get_path(workspace))
        .filter_map(Result::ok)
        .collect();

    if !archive_directory.exists() {
        fs_extra::dir::create(&archive_directory, false).context(format!(
            "Error creating archive directory {}",
            archive_directory.display()
        ))?;
    }

    repository_paths.insert(
        archive_directory
            .canonicalize()
            .context("Error canoncalizing archive directory")?,
    );

    let mut to_archive = Vec::new();
    let mut it = WalkDir::new(workspace).into_iter();

    // I couldn't work out how to use `filter_entry` here, so we just roll our own loop.
    loop {
        let entry = match it.next() {
            None => break,
            Some(Err(err)) => bail!("Error iterating through directory: {}", err),
            Some(Ok(entry)) => entry,
        };
        if repository_paths.contains(entry.path()) {
            it.skip_current_dir();
            continue;
        }
        if entry.path().join(".git").is_dir() {
            to_archive.push(entry.path().to_path_buf());
            it.skip_current_dir();
            continue;
        }
    }

    if !to_archive.is_empty() {
        println!("Archiving {} repositories", to_archive.len());
        for from_dir in to_archive.iter() {
            let relative_dir = from_dir.strip_prefix(workspace)?;
            let to_dir = archive_directory.join(relative_dir);
            println!("Archiving {}", relative_dir.display());
            fs_extra::dir::create_all(&to_dir, true)
                .context(format!("Error creating directory {}", to_dir.display()))?;
            std::fs::rename(&from_dir, &to_dir).context(format!(
                "Error moving directory {} to {}",
                from_dir.display(),
                to_dir.display()
            ))?;
        }
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

fn list(workspace: &PathBuf, full: bool) -> Result<(), Error> {
    let lockfile = Lockfile::new(workspace.join("workspace-lock.toml"));
    let repositories = lockfile.read()?;
    let existing_repositories = repositories.iter().filter(|r| r.exists(&workspace));
    for repo in existing_repositories {
        if full {
            println!("{}", repo.get_path(workspace).unwrap().display());
        } else {
            println!("{}", repo.name());
        }
    }
    Ok(())
}
