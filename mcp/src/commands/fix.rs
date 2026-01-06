use std::collections::HashSet;
use std::fs;
use std::path::Path;

use anyhow::Result;
use colored::*;
use regex::Regex;
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

pub fn run(wikilinks: bool, footer: bool, migrate: bool, check: bool, dry_run: bool, json: bool) -> Result<()> {
    let paths = VaultPaths::new();

    if wikilinks {
        run_wikilinks_fix(&paths, dry_run, json)?;
    } else if footer || migrate || check {
        run_footer_fix(&paths, migrate, check, dry_run, json)?;
    } else {
        if !json {
            println!("{}", "Vault Fix".bold());
            println!("{}", "=".repeat(60));
            println!();
            println!("Available fix options:");
            println!("  --wikilinks   Remove or create missing wikilink targets");
            println!("  --footer      Add missing footer markers");
            println!("  --migrate     Migrate footer to v2 format");
            println!("  --check       Check only (for pre-commit hook)");
            println!();
            println!("Use --help for more information.");
        }
    }

    Ok(())
}

fn run_footer_fix(paths: &VaultPaths, migrate: bool, check: bool, dry_run: bool, json: bool) -> Result<()> {
    let notes = collect_all_notes(paths);
    let mut issues: Vec<FooterIssue> = Vec::new();

    for note in &notes {
        let content = fs::read_to_string(&note.path)?;
        let note_issues = analyze_footer(&content, migrate);
        
        for issue in note_issues {
            issues.push(FooterIssue {
                file: note.name.clone(),
                path: note.path.clone(),
                issue_type: issue,
            });
        }
    }

    if check {
        if issues.is_empty() {
            if !json {
                println!("{}", "✅ All footer markers present".green());
            }
            return Ok(());
        } else {
            if json {
                let result = FixResult {
                    action: "footer-check".to_string(),
                    dry_run: true,
                    fixes_applied: 0,
                    details: issues.iter().map(|i| FixDetail {
                        file: i.file.clone(),
                        issue: format!("{:?}", i.issue_type),
                        fix: "Run vault fix --footer --execute".to_string(),
                        applied: false,
                    }).collect(),
                };
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("{}", "❌ Footer issues found:".red().bold());
                for issue in &issues {
                    println!("  {} - {:?}", issue.file, issue.issue_type);
                }
            }
            std::process::exit(1);
        }
    }

    if issues.is_empty() {
        if json {
            let result = FixResult {
                action: if migrate { "footer-migrate" } else { "footer" }.to_string(),
                dry_run,
                fixes_applied: 0,
                details: Vec::new(),
            };
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            println!("{}", "✅ No footer issues found!".green());
        }
        return Ok(());
    }

    let mut details = Vec::new();
    let mut fixes_applied = 0;

    for issue in &issues {
        let fix_description = match &issue.issue_type {
            FooterIssueType::MissingEnd => "Add <!-- footer_end -->".to_string(),
            FooterIssueType::MissingStart => "Add <!-- footer_start -->".to_string(),
            FooterIssueType::MetadataNeedsMigration => "Convert ### Metadata to <!-- footer_meta -->".to_string(),
        };

        if !dry_run {
            if let Err(e) = apply_footer_fix(&issue.path, &issue.issue_type) {
                details.push(FixDetail {
                    file: issue.file.clone(),
                    issue: format!("{:?}", issue.issue_type),
                    fix: format!("Failed: {}", e),
                    applied: false,
                });
                continue;
            }
            fixes_applied += 1;
        }

        details.push(FixDetail {
            file: issue.file.clone(),
            issue: format!("{:?}", issue.issue_type),
            fix: fix_description,
            applied: !dry_run,
        });
    }

    let result = FixResult {
        action: if migrate { "footer-migrate" } else { "footer" }.to_string(),
        dry_run,
        fixes_applied,
        details,
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        print_footer_report(&result, migrate);
    }

    Ok(())
}

#[derive(Debug, Clone)]
enum FooterIssueType {
    MissingEnd,
    MissingStart,
    MetadataNeedsMigration,
}

struct FooterIssue {
    file: String,
    path: std::path::PathBuf,
    issue_type: FooterIssueType,
}

fn analyze_footer(content: &str, include_migration: bool) -> Vec<FooterIssueType> {
    let mut issues = Vec::new();

    if !content.contains("<!-- footer_end -->") {
        issues.push(FooterIssueType::MissingEnd);
    }

    if include_migration {
        if content.contains("## Footer") && !content.contains("<!-- footer_start -->") {
            issues.push(FooterIssueType::MissingStart);
        }

        if content.contains("### Metadata") && !content.contains("<!-- footer_meta") {
            issues.push(FooterIssueType::MetadataNeedsMigration);
        }
    }

    issues
}

fn apply_footer_fix(path: &Path, issue_type: &FooterIssueType) -> Result<()> {
    let content = fs::read_to_string(path)?;
    let new_content = match issue_type {
        FooterIssueType::MissingEnd => add_footer_end(&content),
        FooterIssueType::MissingStart => add_footer_start(&content),
        FooterIssueType::MetadataNeedsMigration => migrate_metadata(&content),
    };

    if new_content != content {
        fs::write(path, new_content)?;
    }

    Ok(())
}

fn add_footer_end(content: &str) -> String {
    let trimmed = content.trim_end();
    format!("{}\n\n<!-- footer_end -->\n", trimmed)
}

fn add_footer_start(content: &str) -> String {
    if let Some(pos) = content.find("## Footer") {
        let before = &content[..pos];
        let after = &content[pos..];
        format!("{}<!-- footer_start -->\n\n{}", before, after)
    } else {
        content.to_string()
    }
}

fn migrate_metadata(content: &str) -> String {
    let metadata_re = Regex::new(r"(?s)### Metadata\n(.*?)(?=\n<!-- footer_end -->|\n##|\z)").unwrap();
    
    if let Some(caps) = metadata_re.captures(content) {
        let metadata_content = caps.get(1).map_or("", |m| m.as_str());
        let mut yaml_lines = Vec::new();
        
        for line in metadata_content.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("- **") {
                if let Some((key, value)) = rest.split_once("**:") {
                    let key = key.trim().to_lowercase().replace(' ', "_");
                    let value = value.trim();
                    yaml_lines.push(format!("{}: {}", key, value));
                }
            }
        }
        
        if !yaml_lines.is_empty() {
            let yaml_content = yaml_lines.join("\n");
            let footer_meta = format!("<!-- footer_meta\n{}\n-->", yaml_content);
            let new_content = metadata_re.replace(content, &format!("{}\n", footer_meta));
            return new_content.to_string();
        }
    }
    
    content.to_string()
}

fn print_footer_report(result: &FixResult, migrate: bool) {
    println!("{}", "Vault Footer Fix".bold());
    println!("{}", "=".repeat(60));
    println!();

    if result.dry_run {
        println!("{}", "🔍 DRY RUN MODE - No changes made".yellow().bold());
        println!();
    }

    if migrate {
        println!("{}", "Migration mode: v1 → v2 footer format".cyan());
        println!();
    }

    println!("Issues found: {}", result.details.len());
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
        println!("  {} {} - {}", status, detail.file, detail.issue);
    }

    println!();
    println!("{}", "-".repeat(60));

    if result.dry_run {
        println!("Run with {} to apply fixes.", "--execute".cyan());
    } else {
        println!("Fixes applied: {}", result.fixes_applied);
    }
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
