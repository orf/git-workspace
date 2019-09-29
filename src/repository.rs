use console::{strip_ansi_codes, truncate_str};
use failure::Error;
use git2::Repository as Git2Repo;
use indicatif::ProgressBar;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// Eq, Ord and friends are needed to order the list of repositories
#[derive(Deserialize, Serialize, Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct Repository {
    path: String,
    url: String,
    upstream: Option<String>,
    branch: Option<String>,
}

impl Repository {
    pub fn new(
        path: String,
        url: String,
        branch: Option<String>,
        upstream: Option<String>,
    ) -> Repository {
        Repository {
            path,
            url,
            branch,
            upstream,
        }
    }
    pub fn exists(&self, root: &PathBuf) -> bool {
        match Git2Repo::open(root.join(&self.path)) {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    fn set_upstream(&self, root: &PathBuf, upstream: &str) -> Result<(), Error> {
        let repo = Git2Repo::open(root.join(&self.path))?;
        repo.remote("upstream", upstream)?;
        Ok(())
    }

    fn run_with_progress(&self, command: &mut Command, bar: &ProgressBar) -> Result<(), Error> {
        bar.set_message(format!("{}: starting", self.name()).as_str());
        let mut spawned = command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        if let Some(ref mut stderr) = spawned.stderr {
            let lines = BufReader::new(stderr).split('\r' as u8);
            for line in lines {
                let output = line.unwrap();
                if output.is_empty() {
                    continue;
                }
                let line = std::str::from_utf8(&output).unwrap();
                let plain_line = strip_ansi_codes(line);
                let truncated_line = truncate_str(plain_line.trim(), 70, "...");
                bar.set_message(format!("{}: {}", self.name(), truncated_line).as_str());
            }
        }
        spawned.wait()?;
        Ok(())
    }

    pub fn fetch(&self, root: &PathBuf, bar: &ProgressBar) -> Result<(), Error> {
        let mut command = Command::new("git");
        let child = command
            .arg("-C")
            .arg(root.join(&self.path))
            .arg("fetch")
            .arg("--all")
            .arg("--prune")
            .arg("--recurse-submodules=on-demand")
            .arg("--progress");

        self.run_with_progress(child, bar)?;

        Ok(())
    }

    pub fn clone(&self, root: &PathBuf, bar: &ProgressBar) -> Result<(), Error> {
        let mut command = Command::new("git");

        let child = command
            .arg("clone")
            .arg("--recurse-submodules")
            .arg("--progress")
            .arg(&self.url)
            .arg(root.join(&self.path));

        self.run_with_progress(child, bar)?;

        if let Some(upstream) = &self.upstream {
            self.set_upstream(root, upstream.as_str())?;
        }

        Ok(())
    }
    pub fn name(&self) -> String {
        self.path.clone()
    }
    pub fn full_path(&self, root: &Path) -> PathBuf {
        root.join(&self.path)
    }
}
