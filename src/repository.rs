use crate::progress::ProgressSender;
use failure::Error;
use git2::Repository as Git2Repo;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use strip_ansi_escapes;

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

    fn run_with_progress(&self, command: &mut Command, sender: &ProgressSender) -> Result<(), Error> {
        sender.update("starting".to_string());
        let mut spawned = command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        if let Some(ref mut stderr) = spawned.stderr {
            let lines = BufReader::new(stderr).split('\r' as u8).enumerate();
            for (counter, line) in lines {
                let output = line.unwrap();
                if output.is_empty() {
                    continue;
                }
                let plain_bytes = strip_ansi_escapes::strip(output);
                let line = String::from_utf8(plain_bytes.unwrap());
                let mut line = line.unwrap().trim().replace('\n', " ");
                if line.len() >= 70 {
                    line.truncate(70);
                    line.push_str("...");
                }
                sender.update(line.to_string());
            }
        }
        spawned.wait()?;
        Ok(())
    }

    pub fn fetch(&self, root: &PathBuf, sender: &ProgressSender) -> Result<(), Error> {
        let mut command = Command::new("git");
        let child = command
            .arg("-C")
            .arg(root.join(&self.path))
            .arg("fetch")
            .arg("--all")
            .arg("--prune")
            .arg("--recurse-submodules=on-demand")
            .arg("--progress");

        self.run_with_progress(child, sender)?;

        Ok(())
    }

    pub fn clone(&self, root: &PathBuf, sender: &ProgressSender) -> Result<(), Error> {
        let mut command = Command::new("git");

        let child = command
            .arg("clone")
            .arg("--recurse-submodules")
            .arg("--progress")
            .arg(&self.url)
            .arg(root.join(&self.path));

        self.run_with_progress(child, sender)?;

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
