use console::{strip_ansi_codes, truncate_str};
use failure::{Error, ResultExt};
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
        let git_dir = root.join(&self.path).join(".git");
        git_dir.exists() && git_dir.is_dir()
    }

    fn set_upstream(&self, root: &PathBuf) -> Result<(), Error> {
        if self.upstream.is_none() {
            Ok(())
        }
        let upstream = self.upstream.unwrap().as_str();
        let mut command = Command::new("git");
        let child = command
            .arg("-C")
            .arg(root.join(&self.path))
            .arg("remote")
            .arg("set-url")
            .arg("upstream")
            .arg(upstream);

        let output = child.output()?;
        if !output.status.success() {
            bail!("Failed to set upstream on repo {}", root.display())
        }
        Ok(())
    }

    fn run_with_progress(
        &self,
        command: &mut Command,
        progress_bar: &ProgressBar,
    ) -> Result<(), Error> {
        progress_bar.set_message(format!("{}: starting", self.name()).as_str());
        let mut spawned = command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        if let Some(ref mut stderr) = spawned.stderr {
            let lines = BufReader::new(stderr).split(b'\r');
            for line in lines {
                let output = line.unwrap();
                if output.is_empty() {
                    continue;
                }
                let line = std::str::from_utf8(&output).unwrap();
                let plain_line = strip_ansi_codes(line);
                let truncated_line = truncate_str(plain_line.trim(), 70, "...");
                let replaced_line = truncated_line.replace('\n', "");
                progress_bar.set_message(format!("{}: {}", self.name(), replaced_line).as_str());
            }
        }
        let exit_code = spawned.wait()?;
        if !exit_code.success() {
            bail!("Git exited with code {}", exit_code.code().unwrap())
        }
        Ok(())
    }

    pub fn fetch(&self, root: &PathBuf, progress_bar: &ProgressBar) -> Result<(), Error> {
        let mut command = Command::new("git");
        let child = command
            .arg("-C")
            .arg(root.join(&self.path))
            .arg("fetch")
            .arg("--all")
            .arg("--prune")
            .arg("--recurse-submodules=on-demand")
            .arg("--progress");

        self.run_with_progress(child, progress_bar)
            .context(format!("Error fetching repo in {}", root.display()))?;

        Ok(())
    }

    pub fn clone(&self, root: &PathBuf, progress_bar: &ProgressBar) -> Result<(), Error> {
        let mut command = Command::new("git");

        let child = command
            .arg("clone")
            .arg("--recurse-submodules")
            .arg("--progress")
            .arg(&self.url)
            .arg(root.join(&self.path));

        self.run_with_progress(child, progress_bar)
            .context(format!("Error cloning repo into {}", root.display()))?;

        Ok(())
    }
    pub fn name(&self) -> String {
        self.path.clone()
    }
    pub fn full_path(&self, root: &Path) -> PathBuf {
        root.join(&self.path)
    }
}
