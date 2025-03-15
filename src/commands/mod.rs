pub mod add_provider;
pub mod archive;
pub mod completion;
pub mod fetch;
pub mod list;
pub mod lock;
pub mod run;
pub mod switch_and_pull;
pub mod update;

pub use add_provider::add_provider_to_config;
pub use archive::archive;
pub use completion::completion;
pub use fetch::fetch;
pub use list::list;
pub use lock::lock;
pub use run::execute_cmd;
pub use switch_and_pull::pull_all_repositories;
pub use update::update;

use crate::repository::Repository;
use anyhow::{anyhow, Context};
use atomic_counter::{AtomicCounter, RelaxedCounter};
use indicatif::{MultiProgress, ParallelProgressIterator, ProgressBar, ProgressStyle};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use walkdir::WalkDir;

/// Take any number of repositories and apply `f` on each one.
/// This method takes care of displaying progress bars and displaying
/// any errors that may arise.
pub fn map_repositories<F>(repositories: &[Repository], threads: usize, f: F) -> anyhow::Result<()>
where
    F: Fn(&Repository, &ProgressBar) -> anyhow::Result<()> + std::marker::Sync,
{
    // Create our progress bar. We use Arc here as we need to share the MultiProgress across
    // more than 1 thread (described below)
    let progress = Arc::new(MultiProgress::new());
    // Create our total progress bar used with `.progress_iter()`.
    let total_bar = progress.add(ProgressBar::new(repositories.len() as u64));
    total_bar.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {percent}% [{wide_bar:.cyan/blue}] {pos}/{len} (ETA: {eta_precise})").expect("Invalid template")
            .progress_chars("#>-"),
    );

    // user_attended() means a tty is attached to the output.
    let is_attended = console::user_attended();
    let total_repositories = repositories.len();
    // Use a counter here if there is no tty, to show a stream of progress messages rather than
    // a dynamic progress bar.
    let counter = RelaxedCounter::new(1);

    // Create our thread pool. We do this rather than use `.par_iter()` on any iterable as it
    // allows us to customize the number of threads.
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .with_context(|| "Error creating the thread pool")?;

    // pool.install means that `.par_iter()` will use the thread pool we've built above.
    let errors: Vec<(&Repository, anyhow::Error)> = pool.install(|| {
        repositories
            .par_iter()
            // Update our progress bar with each iteration
            .map(|repo| {
                // Create a progress bar and configure some defaults
                let progress_bar = progress.add(ProgressBar::new_spinner());
                progress_bar.set_message("waiting...");
                progress_bar.enable_steady_tick(Duration::from_millis(500));
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
            .progress_with(total_bar)
            // We only care about errors here, so filter them out.
            .filter_map(Result::err)
            // Collect the results into a Vec
            .collect()
    });

    // Print out each repository that failed to run.
    if !errors.is_empty() {
        eprintln!("{} repositories failed:", errors.len());
        for (repo, error) in errors {
            eprintln!("{}:", repo.name());
            error
                .chain()
                .for_each(|cause| eprintln!("because: {}", cause));
        }
    }

    Ok(())
}

/// Find all projects that have been archived or deleted on our providers
pub fn get_all_repositories_to_archive(
    workspace: &Path,
    repositories: Vec<Repository>,
) -> anyhow::Result<Vec<(PathBuf, PathBuf)>> {
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
        fs_extra::dir::create(&archive_directory, false).with_context(|| {
            format!(
                "Error creating archive directory {}",
                archive_directory.display()
            )
        })?;
    }

    // Make sure we add our archive directory to the set of repository paths. This ensures that
    // it's not traversed below!
    repository_paths.insert(
        archive_directory
            .canonicalize()
            .with_context(|| "Error canoncalizing archive directory")?,
    );

    let mut to_archive = Vec::new();
    let mut it = WalkDir::new(workspace).into_iter();

    // Waldir provides a `filter_entry` method, but I couldn't work out how to use it
    // correctly here. So we just roll our own loop:
    loop {
        // Find the next directory. This can throw an error, in which case we bail out.
        // Perhaps we shouldn't bail here?
        let entry = match it.next() {
            None => break,
            Some(Err(err)) => return Err(anyhow!("Error iterating through directory: {}", err)),
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
            let path = entry.path();
            // Find the relative path of the directory from the workspace. So if you have something
            // like `workspace/github/repo-name`, it will be `github/repo-name`.
            let relative_dir = path.strip_prefix(workspace).with_context(|| {
                format!(
                    "Failed to strip the prefix '{}' from {}",
                    workspace.display(),
                    path.display()
                )
            })?;
            // Join the relative directory (`github/repo-name`) with the archive directory.
            let to_dir = archive_directory.join(relative_dir);
            to_archive.push((path.to_path_buf(), to_dir));
            it.skip_current_dir();
            continue;
        }
    }

    Ok(to_archive)
}
