use anyhow::Context;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};

// From https://docs.rs/clt/latest/src/clt/term.rs.html#277-293
fn build_prompt_text(
    text: &str,
    suffix: &str,
    show_default: bool,
    default: Option<&str>,
) -> String {
    let prompt_text = match (default, show_default) {
        (Some(default), true) => format!("{} [{}]", text, default),
        _ => text.to_string(),
    };
    prompt_text + suffix
}

fn get_prompt_input(prompt_text: &str) -> String {
    print!("{}", prompt_text);
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .expect("Failed to read line");
    input.trim_end_matches('\n').to_string()
}

pub fn confirm(text: &str, default: bool, prompt_suffix: &str, show_default: bool) -> bool {
    let default_string = match default {
        true => Some("Y/n"),
        false => Some("y/N"),
    };
    let prompt_text = build_prompt_text(text, prompt_suffix, show_default, default_string);

    loop {
        let prompt_input = get_prompt_input(&prompt_text).to_ascii_lowercase();
        match prompt_input.trim() {
            "y" | "yes" => {
                return true;
            }
            "n" | "no" => {
                return false;
            }
            "" => {
                return default;
            }
            _ => {
                println!("Error: invalid input");
            }
        }
    }
}

// Convert our workspace path to a PathBuf. We cannot use the value given directly as
// it could contain a tilde, so we run `expanduser` on it _if_ we are on a Unix platform.
// On Windows this isn't supported.
#[cfg(unix)]
pub fn expand_workspace_path(path: &Path) -> anyhow::Result<PathBuf> {
    expanduser::expanduser(path.to_string_lossy())
        .with_context(|| "Error expanding git workspace path")
}

#[cfg(not(unix))]
pub fn expand_workspace_path(path: &Path) -> anyhow::Result<PathBuf> {
    Ok(path.to_path_buf())
}

pub fn ensure_workspace_dir_exists(path: &PathBuf) -> anyhow::Result<PathBuf> {
    if !path.exists() {
        fs_extra::dir::create_all(path, false)
            .with_context(|| format!("Error creating workspace directory {}", &path.display()))?;
    }
    path.canonicalize()
        .with_context(|| format!("Error canonicalizing workspace path {}", &path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_prompt_text() {
        // Test with default and show_default true
        assert_eq!(
            build_prompt_text("Continue?", ": ", true, Some("Y/n")),
            "Continue? [Y/n]: "
        );

        // Test with default but show_default false
        assert_eq!(
            build_prompt_text("Continue?", ": ", false, Some("Y/n")),
            "Continue?: "
        );

        // Test without default
        assert_eq!(
            build_prompt_text("Continue?", ": ", true, None),
            "Continue?: "
        );

        // Test with empty text
        assert_eq!(build_prompt_text("", ": ", true, Some("Y/n")), " [Y/n]: ");
    }

    #[test]
    fn test_expand_workspace_path() {
        let path = PathBuf::from("/test/path");
        let result = expand_workspace_path(&path).unwrap();
        assert_eq!(result, path);

        // Test with relative path
        let relative_path = PathBuf::from("test/path");
        let result = expand_workspace_path(&relative_path).unwrap();
        assert_eq!(result, relative_path);
    }

    #[test]
    #[cfg(unix)]
    fn test_expand_workspace_path_on_unix_platform() {
        let custom_home = "/custom/home";
        std::env::set_var("HOME", custom_home);

        let path = PathBuf::from("~/test/path");
        let result = expand_workspace_path(&path).unwrap();
        let expected_path = PathBuf::from(format!("{}/test/path", custom_home));

        assert_eq!(result, expected_path);
        std::env::remove_var("HOME"); // Clean up
    }

    #[test]
    fn test_ensure_workspace_exists() {
        // Test with temporary directory
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().to_path_buf();

        // Test existing directory
        let result = ensure_workspace_dir_exists(&path).unwrap();
        assert_eq!(result, path.canonicalize().unwrap());

        // Test non-existing directory
        let new_path = path.join("new_dir");
        let result = ensure_workspace_dir_exists(&new_path).unwrap();
        assert!(new_path.exists());
        assert_eq!(result, new_path.canonicalize().unwrap());
    }
}
