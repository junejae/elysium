//! Vault initialization commands

use anyhow::Result;
use colored::*;
use std::fs;

use crate::core::config::{Config, CONFIG_FILE_NAME};
use crate::core::paths::{get_vault_root, VaultPaths};

/// Run init command
pub fn run(create: bool, config: bool) -> Result<()> {
    if config {
        return run_config_init();
    }

    run_structure_init(create)
}

/// Generate .elysium.json config file
fn run_config_init() -> Result<()> {
    let vault_root = get_vault_root();
    let config_path = vault_root.join(CONFIG_FILE_NAME);

    println!("{}", "Elysium Configuration Generator".bold());
    println!("{}", "=".repeat(50));
    println!();

    if config_path.exists() {
        println!("{} {} already exists", "!".yellow(), config_path.display());
        println!();
        println!("Options:");
        println!("  1. Edit the existing file manually");
        println!("  2. Delete it and run this command again");
        println!();
        return Ok(());
    }

    let config = Config::default();
    config.save(&vault_root)?;

    println!("{} Created {}", "✓".green(), config_path.display());
    println!();
    println!("{}", "Configuration file structure:".cyan());
    println!();

    // Print abbreviated config preview
    println!("  folders:");
    println!("    notes: \"{}\"", config.folders.notes);
    println!("    projects: \"{}\"", config.folders.projects);
    println!("    archive: \"{}\"", config.folders.archive);
    println!();
    println!("  schema:");
    println!("    types: {:?}", config.schema.types);
    println!("    statuses: {:?}", config.schema.statuses);
    println!("    areas: {:?}", config.schema.areas);
    println!("    max_tags: {}", config.schema.max_tags);
    println!();
    println!(
        "{}",
        "Customize this file to match your vault structure.".dimmed()
    );
    println!();

    Ok(())
}

/// Validate and create vault folder structure
fn run_structure_init(create: bool) -> Result<()> {
    let paths = VaultPaths::new();

    println!("{}", "Second Brain Vault Structure Validator".bold());
    println!("{}", "=".repeat(50));
    println!();

    // Show loaded config info
    let config_path = paths.root.join(CONFIG_FILE_NAME);
    if config_path.exists() {
        println!("{} Using config: {}", "ℹ".cyan(), config_path.display());
    } else {
        println!("{} No config found, using defaults", "ℹ".dimmed());
        println!("  Run {} to create one", "elysium init --config".cyan());
    }
    println!();

    let mut missing = 0;
    let mut created = 0;
    let mut violations = 0;

    println!("{}", "Checking required folders...".cyan());
    println!();

    for (path, purpose, _has_subfolders) in paths.required_folders() {
        if path.exists() {
            println!("{} {} exists ({})", "✓".green(), path.display(), purpose);
        } else if create {
            fs::create_dir_all(path)?;
            created += 1;
            println!("{} Created {} ({})", "✓".green(), path.display(), purpose);
        } else {
            missing += 1;
            println!("{} {} missing ({})", "✗".red(), path.display(), purpose);
        }
    }

    println!();
    println!("{}", "Checking structure violations...".cyan());
    println!();

    violations += check_no_subfolders(&paths.notes)?;
    violations += check_no_subfolders(&paths.projects)?;

    println!();
    println!("{}", "Summary".bold());
    println!("{}", "=".repeat(50));

    if create {
        println!("Created: {} folders", created.to_string().green());
    } else {
        println!(
            "Missing: {} folders",
            if missing > 0 {
                missing.to_string().red()
            } else {
                missing.to_string().green()
            }
        );
    }
    println!(
        "Violations: {}",
        if violations > 0 {
            violations.to_string().red()
        } else {
            violations.to_string().green()
        }
    );
    println!();

    if violations == 0 && missing == 0 {
        println!("{}", "✓ Vault structure is valid!".green());
        Ok(())
    } else if violations > 0 {
        println!(
            "{}",
            "✗ Vault structure has violations. Please fix them.".red()
        );
        std::process::exit(1);
    } else if !create {
        println!(
            "{}",
            "Run with --create to create missing folders.".yellow()
        );
        std::process::exit(1);
    } else {
        Ok(())
    }
}

fn check_no_subfolders(path: &std::path::Path) -> Result<usize> {
    if !path.exists() {
        return Ok(0);
    }

    let mut violations = 0;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            violations += 1;
            println!(
                "{} VIOLATION: Subfolder found in {} (prohibited): {}",
                "✗".red(),
                path.display(),
                entry.path().display()
            );
        }
    }

    Ok(violations)
}
