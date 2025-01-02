mod container;

use container::{GiteaCommit, GiteaContainer};
use git_workspace::commands::{archive, execute_cmd, fetch, lock, update};
use rstest::*;
use std::{
    fs::{read_to_string, remove_dir_all},
    path::Path,
    sync::OnceLock,
};

// Container for gitea test instance, initialized once across all tests
static GITEA_CONTAINER: OnceLock<GiteaContainer> = OnceLock::new();

// Fixture to get or start container if not started
#[fixture]
pub fn gitea_container() -> &'static GiteaContainer {
    GITEA_CONTAINER.get_or_init(GiteaContainer::start)
}

fn update_command(workspace_path: &Path) {
    lock(workspace_path).unwrap();
    update(workspace_path, 8).unwrap();
}

fn execute_command(workspace_path: &Path, cmd: &str, args_raw: &str) {
    let args: Vec<String> = args_raw.split(" ").map(String::from).collect();
    execute_cmd(workspace_path, 8, cmd.to_string(), args).unwrap();
}

#[rstest]
fn test_update_command(gitea_container: &GiteaContainer) {
    // Setup environment
    let (tmp_dir, org_name) = gitea_container.setup();
    let workspace = tmp_dir.path();

    // Test update command
    gitea_container.add_repos(&org_name, ["repo1", "repo2"]);
    update_command(workspace);

    // Check if repo1/2 exists
    let repo1 = format!("{}/repo1/.git/config", org_name);
    let repo2 = format!("{}/repo2/.git/config", org_name);
    assert!(workspace.join(&repo1).exists(), "{} does not exist", &repo1);
    assert!(workspace.join(&repo2).exists(), "{} does not exist", &repo2);

    // Test with new repo add on Gitea server
    gitea_container.add_repos(&org_name, ["repo3"]);
    update_command(workspace);

    // Check if repo3 exists
    let repo3 = format!("{}/repo3/.git/config", org_name);
    assert!(workspace.join(&repo3).exists(), "{} does not exist", &repo3);

    // Test with removed local repo2
    let repo2_path = workspace.join(format!("{}/repo2", org_name));
    remove_dir_all(&repo2_path).unwrap();
    assert!(!repo2_path.exists(),);
    update_command(workspace);

    // Check if repo2 still exists
    assert!(workspace.join(&repo2).exists(), "{} does not exist", &repo2);

    gitea_container.reset(tmp_dir);
}

#[rstest]
fn test_archive_command(gitea_container: &GiteaContainer) {
    // Setup environment
    let (tmp_dir, org_name) = gitea_container.setup();
    let workspace = tmp_dir.path();
    gitea_container.add_repos(&org_name, ["repo1", "repo2", "repo3"]);
    update_command(workspace);

    // Test archive command
    gitea_container.delete_repos(&org_name, ["repo2"]);
    archive(workspace, true).unwrap();

    // Check if .git/config exists for repo2 is in the .archive directory
    let repo2 = workspace.join(format!(".archive/{}/repo2/.git/config", org_name));
    assert!(repo2.exists(), "{} does not exist", repo2.display());

    gitea_container.reset(tmp_dir);
}

#[rstest]
fn test_fetch_and_run_commands(gitea_container: &GiteaContainer) {
    // Setup environment
    let (tmp_dir, org_name) = gitea_container.setup();
    let workspace = tmp_dir.path();
    gitea_container.add_repos(&org_name, ["repo1", "repo2"]);
    update_command(workspace);

    // Test fetch and run commands
    let content = "Hello Orf".to_string();
    let commit = GiteaCommit::new("main", "chore: initial commit", "Hello Orf");
    gitea_container.commit_to_repo(&org_name, "repo1", "README.md", &commit);
    fetch(workspace, 8).unwrap();
    execute_command(workspace, "git", "merge origin/main");

    let org_dir = workspace.join(&org_name).join("repo1");
    println!("Files in {}:", org_dir.display());
    for entry in std::fs::read_dir(org_dir).unwrap() {
        let entry = entry.unwrap();
        println!("{}", entry.path().display());
    }

    // Check that README.md file is present on main branch of repo1
    let repo_path = workspace.join(format!("{}/{}/.git/HEAD", org_name, "repo1"));
    let readme_path = workspace.join(format!("{}/{}/README.md", org_name, "repo1"));
    let branch = read_to_string(&repo_path).unwrap();
    let readme = read_to_string(&readme_path).unwrap();
    assert_eq!(branch.trim(), "ref: refs/heads/main");
    assert_eq!(readme, content);
}
