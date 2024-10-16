use crate::lockfile::Lockfile;
use anyhow::Context;
use std::path::Path;

/// List the contents of our workspace
pub fn list(workspace: &Path, full: bool) -> anyhow::Result<()> {
    // Read and parse the lockfile
    let lockfile = Lockfile::new(workspace.join("workspace-lock.toml"));
    let repositories = lockfile.read().context("Error reading lockfile")?;
    let existing_repositories = repositories.iter().filter(|r| r.exists(workspace));
    for repo in existing_repositories {
        if full {
            println!("{}", repo.get_path(workspace).unwrap().display());
        } else {
            println!("{}", repo.name());
        }
    }
    Ok(())
}
