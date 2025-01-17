use super::lock;
use crate::lockfile::Lockfile;
use crate::utils;
use anyhow::Context;
use console::style;
use std::path::{Path, PathBuf};

use super::get_all_repositories_to_archive;

pub fn archive(workspace: &Path, force: bool) -> anyhow::Result<()> {
    // Archive any repositories that have been deleted from the lockfile.
    lock(workspace)?;

    let lockfile = Lockfile::new(workspace.join("workspace-lock.toml"));
    let repositories = lockfile.read().context("Error reading lockfile")?;
    let repos_to_archive = get_all_repositories_to_archive(workspace, repositories)?;

    if !force {
        for (from_path, to_path) in &repos_to_archive {
            let relative_from_path = from_path.strip_prefix(workspace).unwrap();
            let relative_to_path = to_path.strip_prefix(workspace).unwrap();
            println!(
                "Move {} to {}",
                style(relative_from_path.display()).yellow(),
                style(relative_to_path.display()).green()
            );
        }
        println!(
            "Will archive {} projects",
            style(repos_to_archive.len()).red()
        );
        if repos_to_archive.is_empty() || !utils::confirm("Proceed?", false, " ", true) {
            return Ok(());
        }
    }
    if !repos_to_archive.is_empty() {
        archive_repositories(repos_to_archive)?;
    }
    Ok(())
}

fn archive_repositories(to_archive: Vec<(PathBuf, PathBuf)>) -> anyhow::Result<()> {
    println!("Archiving {} repositories", to_archive.len());
    for (from_dir, to_dir) in to_archive.into_iter() {
        let parent_dir = &to_dir.parent().with_context(|| {
            format!("Failed to get the parent directory of {}", to_dir.display())
        })?;
        // Create all the directories that are needed:
        fs_extra::dir::create_all(parent_dir, false)
            .with_context(|| format!("Error creating directory {}", to_dir.display()))?;

        // Move the directory to the archive directory:
        match std::fs::rename(&from_dir, &to_dir) {
            Ok(_) => {
                println!(
                    "Moved {} to {}",
                    style(from_dir.display()).yellow(),
                    style(to_dir.display()).green()
                );
            }
            Err(e) => {
                eprintln!(
                    "{} {e}\n  Target: {}\n  Dest:   {}\nPlease remove existing directory before retrying",
                    style("Error moving directory!").red(),
                    style(from_dir.display()).yellow(),
                    style(to_dir.display()).green()
                );
            }
        };
    }

    Ok(())
}
