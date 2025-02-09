use base64::{engine::general_purpose, Engine};
use git_workspace::providers::APP_USER_AGENT;
use rand::{distributions::Alphanumeric, Rng};
use reqwest::blocking::{Client, ClientBuilder};
use reqwest::Certificate;
use serde::Serialize;
use ssh_key::{Algorithm::Ed25519, LineEnding, PrivateKey};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::{ExitCode, Termination};
use tempfile::TempDir;
use testcontainers::core::ExecCommand;
use testcontainers::runners::SyncRunner;
use testcontainers::{Container, ImageExt};
use testcontainers_modules::gitea::{Gitea, GITEA_HTTP_PORT, GITEA_SSH_PORT};

pub struct GiteaContainer {
    pub gitea: Container<Gitea>,
    pub url: String,
    pub username: String,
    pub password: String,
    pub private_key: String,
    token: String,
    tls_cert: String,
    http_client: Client,
}

const WORKSPACE_TEMPLATE: &str = r#"[[provider]]
provider = "gitea"
name = "ORG"
url = "https://localhost"
path = "."
env_var = "GITEA_TOKEN"
skip_forks = false
auth_http = true
include = []
exclude = []"#;

const GIT_CONFIG_TEMPLATE: &str = r#"[credential]
  username = "42"
  helper = "!f() { echo 'password=PASSWORD'; }; f"
"#;

#[derive(Serialize)]
pub struct GiteaCommit {
    branch: String,
    content: String,
    message: String,
}

impl GiteaCommit {
    pub fn new(branch: &str, message: &str, content: &str) -> Self {
        let content_base64 = general_purpose::STANDARD.encode(content);
        Self {
            branch: branch.to_string(),
            message: message.to_string(),
            content: content_base64,
        }
    }
}

/// Represents a containerized Gitea instance for testing purposes
/// with pre-configured settings including authentication, TLS,
/// and API access for testing scenarios.
///
/// See Gitea API documentaion:
/// - https://gitea.com/api/swagger#/
impl GiteaContainer {
    fn generate_test_ssh_key() -> (String, String) {
        let private_key = PrivateKey::random(&mut rand::thread_rng(), Ed25519)
            .unwrap_or_else(|e| panic!("Failed to generate key: {}", e));
        let public_key = private_key.public_key();

        // Convert to OpenSSH format strings
        let private_key_str = private_key
            .to_openssh(LineEnding::LF)
            .unwrap_or_else(|e| panic!("Failed to serialize private key: {}", e));
        let public_key_str = public_key
            .to_openssh()
            .unwrap_or_else(|e| panic!("Failed to serialize public key: {}", e));

        (private_key_str.to_string(), public_key_str.to_string())
    }

    /// Starts a new Gitea container instance configured for testing
    ///
    /// This method:
    /// 1. Generates SSH keys for authentication
    /// 2. Creates a Gitea container with:
    ///    - An admin account (user: "42", password: "42")
    ///    - TLS enabled
    ///    - Mapped ports for HTTPS (443) and SSH (22)
    /// 3. Generates an access token with read/write permissions
    ///
    /// Returns a configured GiteaContainer instance ready for testing
    pub fn start() -> Self {
        let (private_key, public_key) = Self::generate_test_ssh_key();
        let (username, password) = ("42".to_string(), "42".to_string());
        let ssh_port = if std::env::var("CI").is_ok() {
            2222
        } else {
            22
        };
        let gitea = Gitea::default()
            .with_admin_account(&username, &password, Some(public_key))
            .with_tls(true)
            .with_mapped_port(443, GITEA_HTTP_PORT)
            .with_mapped_port(ssh_port, GITEA_SSH_PORT)
            .start()
            .unwrap_or_else(|e| panic!("Failed to start Gitea container: {}", e));
        let url = "https://localhost".to_string();

        // Generate token
        let command = ExecCommand::new(vec![
            "/usr/local/bin/gitea",
            "admin",
            "user",
            "generate-access-token",
            "--username",
            &username,
            "--scopes",
            "write:organization,write:user,write:repository",
        ]);

        // Generate access token
        let mut token = String::new();
        gitea
            .exec(command)
            .unwrap_or_else(|e| panic!("to generate access token: {}", e))
            .stdout()
            .read_to_string(&mut token)
            .unwrap();
        let token = token
            .split(":")
            .nth(1)
            .unwrap_or_else(|| panic!("to parse token from output"))
            .trim()
            .to_string();

        // Initialize HTTP client for Gitea API requests
        let tls_cert = gitea.image().tls_ca().unwrap().to_string();
        let cert = Certificate::from_pem(tls_cert.as_bytes()).unwrap();
        let http_client = ClientBuilder::new()
            .https_only(true)
            .user_agent(APP_USER_AGENT)
            .add_root_certificate(cert)
            .use_rustls_tls()
            .build()
            .unwrap();

        Self {
            gitea,
            url,
            username,
            password,
            private_key,
            token,
            tls_cert,
            http_client,
        }
    }

    /// Creates a temporary file in the workspace directory
    ///
    /// The file will be automatically removed when calling the reset function
    pub fn create_tmp_file(&self, tmp_dir: &TempDir, filepath: &str, content: &str) -> PathBuf {
        let config_path = tmp_dir.path().join(filepath);
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let mut file = File::create(&config_path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        config_path
    }

    /// Saves the CA certificate to the temporary directory
    fn save_ca_certificate(&self, tmp_dir: &TempDir) -> PathBuf {
        self.create_tmp_file(tmp_dir, "bundle.pem", self.tls_cert.as_str())
    }

    /// Creates a Git configuration file with test credentials
    fn save_git_config(&self, tmp_dir: &TempDir) {
        let file_content = GIT_CONFIG_TEMPLATE
            .to_string()
            .replace("PASSWORD", &self.token);
        self.create_tmp_file(tmp_dir, "git/config", &file_content);
    }

    fn create_organization(&self, org: &str) {
        #[derive(Serialize)]
        struct CreateOrg {
            username: String,
        }

        let url = format!("{}/api/v1/orgs", self.url);
        self.http_client
            .post(&url)
            .bearer_auth(&self.token)
            .json(&CreateOrg {
                username: org.to_string(),
            })
            .send()
            .unwrap_or_else(|e| panic!("expect to add org {}: {}", org, e))
            .error_for_status()
            .unwrap_or_else(|e| panic!("expect 2xx http response for creating {} org: {}", org, e));
    }

    /// Sets up the test environment for Gitea integration tests
    ///
    /// This method:
    /// 1. Creates a temporary workspace directory for test files
    /// 2. Creates a random organization in Gitea
    /// 3. Create workspace.toml config
    /// 2. Sets up SSL certificate
    /// 3. Configures API authentification by setting GITEA_TOKEN environment variable
    /// 4. Sets up Git authentication with isolated config (no user/system settings)
    ///
    /// See Git documentation for details on isolated config:
    ///   - https://git-scm.com/book/ms/v2/Git-Internals-Environment-Variables
    ///   - https://git-scm.com/docs/git-config#ENVIRONMENT)
    ///
    /// Returns a tuple containing:
    ///   - TempDir: The temporary workspace directory
    ///   - String: The name of the created organization
    pub fn setup(&self) -> (TempDir, String) {
        let tmp_dir = TempDir::new().unwrap();

        let org_name: String = rand::thread_rng()
            .sample_iter(Alphanumeric)
            .take(8) // Adjust length as needed
            .map(char::from)
            .collect();
        self.create_organization(&org_name);

        let config_content = WORKSPACE_TEMPLATE.replace("ORG", &org_name);
        self.create_tmp_file(&tmp_dir, "workspace.toml", &config_content);

        let cert_path = self.save_ca_certificate(&tmp_dir);
        std::env::set_var("SSL_CERT_FILE", cert_path);

        std::env::set_var("GITEA_TOKEN", &self.token);

        self.save_git_config(&tmp_dir);
        std::env::set_var("XDG_CONFIG_HOME", tmp_dir.path());
        std::env::set_var("GIT_SSL_NO_VERIFY", "true");
        std::env::set_var("GIT_CONFIG_NOSYSTEM", "true");

        println!(
            "\nCreate org {} and tmp workspace directory {}",
            &org_name,
            tmp_dir.path().display(),
        );

        (tmp_dir, org_name)
    }

    /// Creates multiple repositories on the Gitea organization
    pub fn add_repos<T, R>(&self, org_name: &str, repos: R)
    where
        T: AsRef<str>,
        R: IntoIterator<Item = T>,
    {
        #[derive(Serialize)]
        struct CreateRepo {
            name: String,
        }

        let url = format!("{}/api/v1/orgs/{}/repos", self.url, org_name);
        for repo in repos {
            self.http_client
                .post(&url)
                .bearer_auth(&self.token)
                .json(&CreateRepo {
                    name: repo.as_ref().to_string(),
                })
                .send()
                .unwrap_or_else(|e| panic!("expect to add repo {}: {}", repo.as_ref(), e))
                .error_for_status()
                .unwrap_or_else(|e| {
                    panic!(
                        "expect 2xx http response for creating {} repo: {}",
                        repo.as_ref(),
                        e,
                    )
                });
        }
    }

    /// Deletes multiple repositories from the Gitea organization
    pub fn delete_repos<T, R>(&self, org_name: &str, repos: R)
    where
        T: AsRef<str>,
        R: IntoIterator<Item = T>,
    {
        for repo in repos {
            let url = format!("{}/api/v1/repos/{}/{}", self.url, org_name, repo.as_ref());
            self.http_client
                .delete(&url)
                .bearer_auth(&self.token)
                .send()
                .unwrap_or_else(|e| panic!("expect to delete repo {}: {}", repo.as_ref(), e))
                .error_for_status()
                .unwrap_or_else(|e| {
                    panic!(
                        "expect 2xx http response for deleting {} repo: {}",
                        repo.as_ref(),
                        e,
                    )
                });
        }
    }

    /// Creates a new commit in the specified repository with the given file contents
    pub fn commit_to_repo(&self, org_name: &str, repo: &str, filepath: &str, body: &GiteaCommit) {
        let url = format!(
            "{}/api/v1/repos/{}/{}/contents/{}",
            self.url, org_name, repo, filepath
        );
        self.http_client
            .post(&url)
            .bearer_auth(&self.token)
            .json(body)
            .send()
            .unwrap_or_else(|e| panic!("expect to create new commit for repo {}: {}", repo, e))
            .error_for_status()
            .unwrap_or_else(|e| {
                panic!(
                    "expect 2xx http response when creating new commit on {} repo: {}",
                    repo, e,
                )
            });
    }

    /// Resets the test environment by removing  the temporary folder on the system.
    ///
    /// Notes:
    ///  - organization and repositories created during tests are not removed
    ///    as they use unique names
    ///  - environment variables are not cleared
    pub fn reset(&self, tmp_dir: TempDir) {
        tmp_dir.close().unwrap();
    }
}

impl Termination for &'static GiteaContainer {
    fn report(self) -> ExitCode {
        ExitCode::SUCCESS
    }
}
