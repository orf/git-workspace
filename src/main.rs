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
use std::sync::Arc;
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
    // Parse our arguments to Args using structopt.
    let args = Args::from_args();
    // Call handle_main and collect any errors. If we have an Err object then we
    // print out a message with a chain of contexts, which should be informative.
    if let Err(e) = handle_main(args) {
        eprintln!("{}", style("There was an internal error!").red());
        for cause in e.iter_chain() {
            eprintln!("{}", style(cause).red());
        }
        process::exit(1);
    }
}

/// Our actual main function.
fn handle_main(args: Args) -> Result<(), Error> {
    // Convert our workspace path to a PathBuf. We cannot use the value given directly as
    // it could contain a tilde, so we run `expanduser` on it _if_ we are on a Unix platform.
    // On Windows this isn't supported.
    let expanded_workspace_path;
    #[cfg(not(unix))]
    {
        expanded_workspace_path = PathBuf::from(args.workspace);
    }
    #[cfg(unix)]
    {
        expanded_workspace_path = expanduser::expanduser(args.workspace.to_string_lossy())
            .context("Error expanding git workspace path")?;
    }

    // If our workspace path doesn't exist then we need to create it, and call `canonicalize`
    // on the result. This fails if the path does not exist.
    let workspace_path = (if expanded_workspace_path.exists() {
        &expanded_workspace_path
    } else {
        fs_extra::dir::create_all(&expanded_workspace_path, false).context(format!(
            "Error creating workspace directory {}",
            &expanded_workspace_path.display()
        ))?;
        println!(
            "Created {} as it did not exist",
            &expanded_workspace_path.display()
        );

        &expanded_workspace_path
    })
    .canonicalize()
    .context(format!(
        "Error canonicalizing workspace path {}",
        &expanded_workspace_path.display()
    ))?;

    // Run our sub command. Pretty self-explanatory.
    match args.command {
        Command::List { full } => list(&workspace_path, full)?,
        Command::Update { threads } => {
            lock(&workspace_path)?;
            update(&workspace_path, threads)?
        }
        Command::Fetch { threads } => fetch(&workspace_path, threads)?,
        Command::Add(provider) => add_provider_to_config(&workspace_path, provider)?,
    };
    Ok(())
}

/// Add a given ProviderSource to our configuration file.
fn add_provider_to_config(
    workspace: &PathBuf,
    provider_source: ProviderSource,
) -> Result<(), Error> {
    // Load and parse our configuration file
    let config = Config::new(workspace.join("workspace.toml"));
    let mut sources = config.read().context("Error reading config file")?;
    // Ensure we don't add duplicates:
    if sources.iter().any(|s| s == &provider_source) {
        println!("Entry already exists, skipping");
    } else {
        // Push the provider into the source and write it to the configuration file
        sources.push(provider_source);
        config.write(sources).context("Error writing config file")?;
        println!("Added entry to workspace.toml");
    }
    Ok(())
}

/// Update our workspace. This clones any new repositories and archives old ones.
fn update(workspace: &PathBuf, threads: usize) -> Result<(), Error> {
    // Load our lockfile
    let lockfile = Lockfile::new(workspace.join("workspace-lock.toml"));
    let repositories = lockfile.read().context("Error reading lockfile")?;

    println!("Updating {} repositories", repositories.len());

    map_repositories(&repositories, threads, |r, progress_bar| {
        // Only clone repositories that don't exist
        if !r.exists(workspace) {
            r.clone(&workspace, &progress_bar)?;
            // Maybe this should always be run, but whatever. It's fine for now.
            r.set_upstream(&workspace)?;
        }
        Ok(())
    })?;
    // Archive any repositories that have been deleted from the lockfile.
    archive_repositories(workspace, repositories).context("Error archiving repositories")?;

    Ok(())
}

/// Run `git fetch` on all our repositories
fn fetch(workspace: &PathBuf, threads: usize) -> Result<(), Error> {
    // Read the lockfile
    let lockfile = Lockfile::new(workspace.join("workspace-lock.toml"));
    let repositories = lockfile.read()?;

    // We only care about repositories that exist
    let repos_to_fetch: Vec<Repository> = repositories
        .iter()
        .filter(|r| r.exists(workspace))
        .cloned()
        .collect();

    println!("Fetching {} repositories", repos_to_fetch.len(),);

    // Run fetch on them
    map_repositories(&repos_to_fetch, threads, |r, progress_bar| {
        r.fetch(&workspace, &progress_bar)
    })?;

    Ok(())
}

/// Update our lockfile
fn lock(workspace: &PathBuf) -> Result<(), Error> {
    // Read the configuration sources
    let config = Config::new(workspace.join("workspace.toml"));
    let sources = config.read()?;
    // For each source, in sequence, fetch the repositories
    let mut all_repositories = vec![];
    for source in sources.iter() {
        all_repositories.extend(source.fetch_repositories()?);
    }
    // We may have duplicated repositories here. Make sure they are unique based on the full path.
    all_repositories.sort();
    all_repositories.dedup();
    // Write the lockfile out
    let lockfile = Lockfile::new(workspace.join("workspace-lock.toml"));
    lockfile.write(&all_repositories)?;
    Ok(())
}

/// List the contents of our workspace
fn list(workspace: &PathBuf, full: bool) -> Result<(), Error> {
    // Read and parse the lockfile
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

/// Take any number of repositories and apply `f` on each one.
/// This method takes care of displaying progress bars and displaying
/// any errors that may arise.
fn map_repositories<F>(repositories: &[Repository], threads: usize, f: F) -> Result<(), Error>
where
    F: Fn(&Repository, &ProgressBar) -> Result<(), Error> + std::marker::Sync,
{
    // Create our progress bar. We use Arc here as we need to share the MutliProgress across
    // more than 1 thread (described below)
    let progress = Arc::new(MultiProgress::new());
    // Create our total progress bar used with `.progress_iter()`.
    let total_bar = progress.add(ProgressBar::new(repositories.len() as u64));
    total_bar.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {percent}% [{wide_bar:.cyan/blue}] {pos}/{len} (ETA: {eta_precise})")
            .progress_chars("#>-"),
    );

    // user_attended() means a tty is attached to the output.
    let is_attended = console::user_attended();
    let total_repositories = repositories.len();
    // Use a counter here if there is no tty, to show a stream of progress messages rather than
    // a dynamic progress bar.
    let counter = RelaxedCounter::new(1);

    // Clone our Arc<MultiProgress> and spawn a thread. We need to call `.join()` on the
    // `MultiProgress` to ensure that messages are pumped and the progress bars are updated.
    // We need to do this in a thread as the `.map()` we do below also blocks.
    let progress_wait = progress.clone();
    let waiting_thread: JoinHandle<std::result::Result<(), Error>> = thread::spawn(move || {
        progress_wait.join()?;
        Ok(())
    });

    // Create our thread pool. We do this rather than use `.par_iter()` on any iterable as it
    // allows us to customize the number of threads.
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .context("Error creating the thread pool")?;

    // pool.install means that `.par_iter()` will use the thread pool we've built above.
    let errors: Vec<(&Repository, Error)> = pool.install(|| {
        repositories
            .par_iter()
            // Update our progress bar with each iteration
            .progress_with(total_bar)
            .map(|repo| {
                // Create a progress bar and configure some defaults
                let progress_bar = progress.add(ProgressBar::new_spinner());
                progress_bar.set_message("waiting...");
                progress_bar.enable_steady_tick(500);
                // Increment our counter for use if the console is not a tty.
                let idx = counter.inc();
                if !is_attended {
                    println!("[{}/{}] Starting {}", idx, total_repositories, repo.name());
                }
                // Run our given function. If the result is an error then attach the
                // erroring Repository object to it.
                let result = match f(repo, &progress_bar) {
                    Ok(_) => Ok(()),
                    Err(e) => Err((repo, e)),
                };
                if !is_attended {
                    println!("[{}/{}] Finished {}", idx, total_repositories, repo.name());
                }
                // Clear the progress bar and return the result
                progress_bar.finish_and_clear();
                result
            })
            // We only care about errors here, so filter them out.
            .filter_map(Result::err)
            // Collect the results into a Vec
            .collect()
    });

    // Join the progress thread. This will never join if the `progress_bar.finish_and_clear()`
    // is not called on every progress bar, but that should never happen.
    waiting_thread.join();

    // Print out each repository that failed to run.
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

/// Find all projects that have been archived or deleted on our providers
fn archive_repositories(workspace: &PathBuf, repositories: Vec<Repository>) -> Result<(), Error> {
    // The logic here is as follows:
    // 1. Iterate through all directories. If it's a "safe" directory (one that contains a project
    //    in our lockfile), we skip it entirely.
    // 2. If the directory is not, and contains a `.git` directory, then we mark it for archival and
    //    skip processing.
    // This assumes nobody deletes a .git directory in one of their projects.

    // Windows doesn't like .archive.
    let archive_directory = if cfg!(windows) {
        workspace.join("_archive")
    } else {
        workspace.join(".archive")
    };

    // Create a set of all repository paths that currently exist.
    let mut repository_paths: HashSet<PathBuf> = repositories
        .iter()
        .filter(|r| r.exists(workspace))
        .map(|r| r.get_path(workspace))
        .filter_map(Result::ok)
        .collect();

    // If the archive directory does not exist then we create it
    if !archive_directory.exists() {
        fs_extra::dir::create(&archive_directory, false).context(format!(
            "Error creating archive directory {}",
            archive_directory.display()
        ))?;
    }

    // Make sure we add our archive directory to the set of repository paths. This ensures that
    // it's not traversed below!
    repository_paths.insert(
        archive_directory
            .canonicalize()
            .context("Error canoncalizing archive directory")?,
    );

    // Create a vector of all repositories to archive, and WalkDir iterator
    let mut to_archive = Vec::new();
    let mut it = WalkDir::new(workspace).into_iter();

    // Waldir provides a `filter_entry` method, but I couldn't work out how to use it
    // correctly here. So we just roll our own loop:
    loop {
        // Find the next directory. This can throw an error, in which case we bail out.
        // Perhaps we shouldn't bail here?
        let entry = match it.next() {
            None => break,
            Some(Err(err)) => bail!("Error iterating through directory: {}", err),
            Some(Ok(entry)) => entry,
        };
        // If the current path is in the set of repository paths then we skip processing it entirely.
        if repository_paths.contains(entry.path()) {
            it.skip_current_dir();
            continue;
        }
        // If the entry has a .git directory inside it then we add it to the `to_archive` list
        // and skip the current directory.
        if entry.path().join(".git").is_dir() {
            to_archive.push(entry.path().to_path_buf());
            it.skip_current_dir();
            continue;
        }
    }

    if !to_archive.is_empty() {
        println!("Archiving {} repositories", to_archive.len());
        for from_dir in to_archive.iter() {
            // Find the relative path of the directory from the workspace. So if you have something
            // like `workspace/github/repo-name`, it will be `github/repo-name`.
            let relative_dir = from_dir.strip_prefix(workspace)?;
            // Join the relative directory (`github/repo-name`) with the archive directory.
            let to_dir = archive_directory.join(relative_dir);
            println!("Archiving {}", relative_dir.display());
            // Create all the directories that are needed:
            fs_extra::dir::create_all(&to_dir.parent().unwrap(), false)
                .context(format!("Error creating directory {}", to_dir.display()))?;
            // Move the directory to the archive directory:
            std::fs::rename(&from_dir, &to_dir).context(format!(
                "Error moving directory {} to {}",
                from_dir.display(),
                to_dir.display()
            ))?;
        }
    }

    Ok(())
}
