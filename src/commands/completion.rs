use std::io;

use clap::Command;
use clap_complete::{generate, Shell};

/// Generate shell completions
pub fn completion(shell: Shell, app: &mut Command) -> anyhow::Result<()> {
    generate(shell, app, app.get_name().to_string(), &mut io::stdout());
    Ok(())
}
