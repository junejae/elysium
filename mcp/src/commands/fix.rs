use std::collections::HashSet;
use std::fs;
use std::path::Path;

use anyhow::Result;
use colored::*;
use serde::Serialize;

use crate::core::note::{collect_all_notes, collect_note_names};
use crate::core::paths::VaultPaths;

#[derive(Serialize)]
struct FixResult {
    action: String,
    dry_run: bool,
    fixes_applied: usize,
    details: Vec<FixDetail>,
}

#[derive(Serialize)]
struct FixDetail {
    file: String,
    issue: String,
    fix: String,
    applied: bool,
}

pub fn run(wikilinks: bool, dry_run: bool, json: bool) -> Result<()> {
    let paths = VaultPaths::new();

    if wikilinks {
        run_wikilinks_fix(&paths, dry_run, json)?;
    } else {
        if !json {
            println!("{}", "Vault Fix".bold());
            println!("{}", "=".repeat(60));
            println!();
            println!("Available fix options:");
            println!("  --wikilinks   Remove or create missing wikilink targets");
            println!();
            println!("Use --help for more information.");
        }
    }

    Ok(())
}

fn run_wikilinks_fix(paths: &VaultPaths, dry_run: bool, json: bool) -> Result<()> {
    let notes = collect_all_notes(paths);
    let note_names = collect_note_names(paths);

    let mut broken_links: Vec<(String, String, String)> = Vec::new();

    for note in &notes {
        let links = note.wikilinks();
        for link in links {
            if !note_names.contains(&link) {
                broken_links.push((
                    note.name.clone(),
                    note.path.to_string_lossy().to_string(),
                    link,
                ));
            }
        }
    }

    if broken_links.is_empty() {
        if json {
            let result = FixResult {
                action: "wikilinks".to_string(),
                dry_run,
                fixes_applied: 0,
                details: Vec::new(),
            };
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            println!("{}", "✅ No broken wikilinks found!".green());
        }
        return Ok(());
    }

    let unique_broken: HashSet<_> = broken_links
        .iter()
        .map(|(_, _, link)| link.clone())
        .collect();
    let mut details = Vec::new();
    let mut fixes_applied = 0;

    for (note_name, note_path, link) in &broken_links {
        let fix_description = format!("Remove [[{}]] from {}", link, note_name);

        if !dry_run {
            if let Err(e) = remove_wikilink_from_file(Path::new(note_path), link) {
                details.push(FixDetail {
                    file: note_name.clone(),
                    issue: format!("Broken link: [[{}]]", link),
                    fix: format!("Failed: {}", e),
                    applied: false,
                });
                continue;
            }
            fixes_applied += 1;
        }

        details.push(FixDetail {
            file: note_name.clone(),
            issue: format!("Broken link: [[{}]]", link),
            fix: fix_description,
            applied: !dry_run,
        });
    }

    let result = FixResult {
        action: "wikilinks".to_string(),
        dry_run,
        fixes_applied,
        details,
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        print_wikilink_report(&result, &unique_broken);
    }

    Ok(())
}

fn remove_wikilink_from_file(path: &Path, target: &str) -> Result<()> {
    let content = fs::read_to_string(path)?;

    let pattern_simple = format!("[[{}]]", target);
    let new_content = content.replace(&pattern_simple, target);

    let pattern_display =
        regex::Regex::new(&format!(r"\[\[{}\|([^\]]+)\]\]", regex::escape(target)))?;
    let new_content = pattern_display.replace_all(&new_content, "$1").to_string();

    if new_content != content {
        fs::write(path, new_content)?;
    }

    Ok(())
}

fn print_wikilink_report(result: &FixResult, unique_broken: &HashSet<String>) {
    println!("{}", "Vault Wikilink Fix".bold());
    println!("{}", "=".repeat(60));
    println!();

    if result.dry_run {
        println!("{}", "🔍 DRY RUN MODE - No changes made".yellow().bold());
        println!();
    }

    println!("Broken wikilinks found: {}", unique_broken.len());
    println!();

    println!("{}", "Unique broken targets:".cyan());
    for link in unique_broken {
        println!("  • [[{}]]", link.red());
    }
    println!();

    println!("{}", "Fix actions:".cyan());
    for detail in &result.details {
        let status = if result.dry_run {
            "[WOULD FIX]".yellow()
        } else if detail.applied {
            "[FIXED]".green()
        } else {
            "[FAILED]".red()
        };
        println!("  {} {} in {}", status, detail.issue, detail.file);
    }

    println!();
    println!("{}", "-".repeat(60));

    if result.dry_run {
        println!("Run with {} to apply fixes.", "--execute".cyan());
    } else {
        println!("Fixes applied: {}", result.fixes_applied);
    }
}
