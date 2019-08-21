use git2::Repository as Git2Repo;
use std::path::Path;
use std::process::Command;

pub struct CloneError {}

//#[derive(Deserialize, Serialize, Debug)]
pub struct Repository {
    path: String,
    url: String,
    branch: String,
}

impl Repository {
    pub fn new(path: String, url: String, branch: String) -> Repository {
        Repository { path, url, branch }
    }
    pub fn exists(&self, root: &Path) -> bool {
        match Git2Repo::open(root.join(&self.path)) {
            Ok(_) => true,
            Err(_) => false,
        }
    }
    pub fn clone(&self, root: &Path) {
        let mut command = Command::new("git");

        println!(
            "{:?}",
            command
                .arg("clone")
                .arg("--recurse-submodules")
                .arg("--progress")
                .arg(&self.url)
                .arg(root.join(&self.path))
                .output()
        );
    }
}
