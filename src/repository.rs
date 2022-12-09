use anyhow::{anyhow, Context};
use console::{strip_ansi_codes, truncate_str};
use git2::build::CheckoutBuilder;
use git2::{Repository as Git2Repository, StatusOptions};
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
    pub upstream: Option<String>,
    pub branch: Option<String>,
}

impl Repository {
    pub fn new(
        path: String,
        url: String,
        branch: Option<String>,
        upstream: Option<String>,
    ) -> Repository {
        // We have to normalize repository names here. On windows if you do `path.join(self.name())`
        // it will cause issues if the name contains a forward slash. So here we just normalize it
        // to the path separator on the system.
        let norm_path = if cfg!(windows) {
            path.replace('/', std::path::MAIN_SEPARATOR.to_string().as_str())
        } else {
            path
        };

        Repository {
            path: norm_path,
            url,
            branch,
            upstream,
        }
    }

    pub fn set_upstream(&self, root: &Path) -> anyhow::Result<()> {
        let upstream = match &self.upstream {
            Some(upstream) => upstream,
            None => return Ok(()),
        };

        let mut command = Command::new("git");
        let child = command
            .arg("-C")
            .arg(root.join(self.name()))
            .arg("remote")
            .arg("rm")
            .arg("upstream")
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        child.status()?;

        let mut command = Command::new("git");
        let child = command
            .arg("-C")
            .arg(root.join(self.name()))
            .arg("remote")
            .arg("add")
            .arg("upstream")
            .arg(upstream);

        let output = child.output()?;
        if !output.status.success() {
            let stderr =
                std::str::from_utf8(&output.stderr).with_context(|| "Error decoding git output")?;
            return Err(anyhow!(
                "Failed to set upstream on repo {}: {}",
                root.display(),
                stderr.trim()
            ));
        }
        Ok(())
    }

    fn run_with_progress(
        &self,
        command: &mut Command,
        progress_bar: &ProgressBar,
    ) -> anyhow::Result<()> {
        progress_bar.set_message(format!("{}: starting", self.name()));
        let mut spawned = command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("Error starting command {:?}", command))?;

        let mut last_line = format!("{}: running...", self.name());
        progress_bar.set_message(last_line.clone());

        if let Some(ref mut stderr) = spawned.stderr {
            let lines = BufReader::new(stderr).split(b'\r');
            for line in lines {
                let output = line.unwrap();
                if output.is_empty() {
                    continue;
                }
                let line = std::str::from_utf8(&output).unwrap();
                let plain_line = strip_ansi_codes(line).replace('\n', " ");
                let truncated_line = truncate_str(plain_line.trim(), 70, "...");
                progress_bar.set_message(format!("{}: {}", self.name(), truncated_line));
                last_line = plain_line;
            }
        }
        let exit_code = spawned
            .wait()
            .context("Error waiting for process to finish")?;
        if !exit_code.success() {
            return Err(anyhow!(
                "Git exited with code {}: {}",
                exit_code.code().unwrap(),
                last_line
            ));
        }
        Ok(())
    }

    pub fn execute_cmd(
        &self,
        root: &Path,
        progress_bar: &ProgressBar,
        cmd: &str,
        args: &[String],
    ) -> anyhow::Result<()> {
        let mut command = Command::new(cmd);
        let child = command.args(args).current_dir(root.join(self.name()));

        self.run_with_progress(child, progress_bar)
            .with_context(|| format!("Error running command in repo {}", self.name()))?;

        Ok(())
    }

    pub fn switch_to_primary_branch(&self, root: &Path) -> anyhow::Result<()> {
        let branch = match &self.branch {
            None => return Ok(()),
            Some(b) => b,
        };
        let repo = Git2Repository::init(root.join(self.name()))?;
        let status = repo.statuses(Some(&mut StatusOptions::default()))?;
        if !status.is_empty() {
            return Err(anyhow!(
                "Repository is dirty, cannot switch to branch {}",
                branch
            ));
        }
        repo.set_head(&format!("refs/heads/{}", branch))
            .with_context(|| format!("Cannot find branch {}", branch))?;
        repo.checkout_head(Some(CheckoutBuilder::default().safe().force()))
            .with_context(|| format!("Error checking out branch {}", branch))?;
        Ok(())
    }

    pub fn clone(&self, root: &Path, progress_bar: &ProgressBar) -> anyhow::Result<()> {
        let mut command = Command::new("git");

        let child = command
            .arg("clone")
            .arg("--recurse-submodules")
            .arg("--progress")
            .arg(&self.url)
            .arg(root.join(self.name()));

        self.run_with_progress(child, progress_bar)
            .with_context(|| {
                format!("Error cloning repo into {} from {}", self.name(), &self.url)
            })?;

        Ok(())
    }
    pub fn name(&self) -> &String {
        &self.path
    }
    pub fn get_path(&self, root: &Path) -> anyhow::Result<PathBuf> {
        let joined = root.join(self.name());
        joined
            .canonicalize()
            .with_context(|| format!("Cannot resolve {}", joined.display()))
    }
    pub fn exists(&self, root: &Path) -> bool {
        match self.get_path(root) {
            Ok(path) => {
                let git_dir = root.join(path).join(".git");
                git_dir.exists() && git_dir.is_dir()
            }
            Err(_) => false,
        }
    }
}
