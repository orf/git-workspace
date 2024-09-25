use crate::config::{Config, ProviderSource};
use anyhow::{anyhow, Context};
use console::style;
use std::path::Path;

/// Add a given ProviderSource to our configuration file.
pub fn add_provider_to_config(
    workspace: &Path,
    provider_source: ProviderSource,
    file: &Path,
) -> anyhow::Result<()> {
    if !provider_source.correctly_configured() {
        return Err(anyhow!("Provider is not correctly configured"));
    }
    let path_to_config = workspace.join(file);
    // Load and parse our configuration files
    let config = Config::new(vec![path_to_config]);
    let mut sources = config.read().with_context(|| "Error reading config file")?;
    // Ensure we don't add duplicates:
    if sources.iter().any(|s| s == &provider_source) {
        println!("Entry already exists, skipping");
    } else {
        println!(
            "Adding {} to {}",
            provider_source,
            style(&workspace.join(file).display()).green()
        );
        // Push the provider into the source and write it to the configuration file
        sources.push(provider_source);
        config
            .write(sources, &workspace.join(file))
            .with_context(|| "Error writing config file")?;
    }
    Ok(())
}
