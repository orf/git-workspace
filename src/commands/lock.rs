use crate::config::Config;
use crate::lockfile::Lockfile;
use crate::repository::Repository;
use anyhow::Context;
use indicatif::ParallelProgressIterator;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::path::Path;

/// Update our lockfile
pub fn lock(workspace: &Path) -> anyhow::Result<()> {
    let config = Config::from_workspace(workspace)?;

    // Read the configuration sources
    let sources = config
        .read()
        .with_context(|| "Error reading config files")?;

    let total_bar = ProgressBar::new(sources.len() as u64);
    total_bar.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {percent}% [{wide_bar:.cyan/blue}] {pos}/{len} (ETA: {eta_precise})").expect("Invalid template")
            .progress_chars("#>-"),
    );

    println!("Fetching repositories...");

    // For each source, in sequence, fetch the repositories
    let results = sources
        .par_iter()
        .map(|source| {
            source
                .fetch_repositories()
                .with_context(|| format!("Error fetching repositories from {}", source))
        })
        .progress_with(total_bar)
        .collect::<anyhow::Result<Vec<_>>>()?;
    let mut all_repositories: Vec<Repository> = results.into_iter().flatten().collect();
    // let all_repositories: Vec<Repository> = all_repository_results.iter().collect::<anyhow::Result<Vec<Repository>>>()?;
    // We may have duplicated repositories here. Make sure they are unique based on the full path.
    all_repositories.sort();
    all_repositories.dedup();
    // Write the lockfile out
    let lockfile = Lockfile::new(workspace.join("workspace-lock.toml"));
    lockfile.write(&all_repositories)?;
    Ok(())
}
