//! Vault initialization commands

use anyhow::Result;
use colored::*;
use std::fs;

use crate::core::config::{Config, PLUGIN_CONFIG_PATH};
use crate::core::paths::get_vault_root;

pub fn run(config: bool, inbox: Option<String>) -> Result<()> {
    if config || inbox.is_some() {
        return run_config_init(inbox);
    }

    println!("{}", "Elysium Initialization".bold());
    println!("{}", "=".repeat(50));
    println!();
    println!("Usage:");
    println!(
        "  {} - Create config with default inbox (inbox.md)",
        "elysium init --config".cyan()
    );
    println!(
        "  {} - Create config with custom inbox path",
        "elysium init --inbox <path>".cyan()
    );
    println!();
    println!("Examples:");
    println!("  elysium init --config");
    println!("  elysium init --inbox \"Inbox/inbox.md\"");
    println!("  elysium init --config --inbox \"quick-capture.md\"");
    println!();

    Ok(())
}

fn run_config_init(inbox: Option<String>) -> Result<()> {
    let vault_root = get_vault_root();
    let config_path = vault_root.join(PLUGIN_CONFIG_PATH);

    println!("{}", "Elysium Configuration Generator".bold());
    println!("{}", "=".repeat(50));
    println!();

    let mut config = Config::load(&vault_root);

    if let Some(inbox_path) = &inbox {
        config.inbox.path = inbox_path.clone();
    }

    let existed = config_path.exists();
    config.save(&vault_root)?;

    if existed {
        println!("{} Updated {}", "✓".green(), config_path.display());
    } else {
        println!("{} Created {}", "✓".green(), config_path.display());
    }

    let inbox_path = vault_root.join(&config.inbox.path);
    if !inbox_path.exists() {
        if let Some(parent) = inbox_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
                println!("{} Created directory {}", "✓".green(), parent.display());
            }
        }

        let inbox_content =
            "# Inbox\n\n> Quick capture space. Process with AI or manually.\n\n---\n\n";
        fs::write(&inbox_path, inbox_content)?;
        println!("{} Created {}", "✓".green(), inbox_path.display());
    } else {
        println!(
            "{} Inbox already exists: {}",
            "→".blue(),
            inbox_path.display()
        );
    }

    println!();
    println!("{}", "Configuration:".cyan());
    println!();
    println!("  schema:");
    println!("    types: {:?}", config.schema.types);
    println!("    statuses: {:?}", config.schema.statuses);
    println!("    areas: {:?}", config.schema.areas);
    println!("    max_tags: {}", config.schema.max_tags);
    println!();
    println!("  folders:");
    println!("    notes: \"{}\"", config.folders.notes);
    println!("    projects: \"{}\"", config.folders.projects);
    println!("    archive: \"{}\"", config.folders.archive);
    println!();
    println!("  inbox:");
    println!("    enabled: {}", config.inbox.enabled);
    println!("    path: \"{}\"", config.inbox.path);
    println!();
    println!(
        "{}",
        "Config saved to plugin path. Edit via Obsidian settings or config.json directly.".dimmed()
    );
    println!();

    Ok(())
}
