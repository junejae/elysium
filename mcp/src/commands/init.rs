use anyhow::Result;
use colored::*;
use std::fs;

use crate::core::paths::VaultPaths;

pub fn run(create: bool) -> Result<()> {
    let paths = VaultPaths::new();

    println!("{}", "Second Brain Vault Structure Validator".bold());
    println!("{}", "=".repeat(50));
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
