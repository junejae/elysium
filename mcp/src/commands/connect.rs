//! Connect orphan notes to related notes via wikilinks
//!
//! This command identifies orphan notes (notes with no incoming links)
//! and automatically adds wikilinks to connect them with related notes.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use colored::*;
use serde::Serialize;

use crate::core::note::{collect_all_notes, Note};
use crate::core::paths::VaultPaths;
use crate::search::engine::SearchEngine;

#[derive(Serialize)]
struct ConnectResult {
    dry_run: bool,
    orphan_count: usize,
    connected_count: usize,
    connections: Vec<ConnectionDetail>,
}

#[derive(Serialize)]
struct ConnectionDetail {
    orphan: String,
    related_notes: Vec<String>,
    method: String,
    applied: bool,
}

pub fn run(
    dry_run: bool,
    min_tags: Option<usize>,
    semantic: bool,
    limit: Option<usize>,
    json: bool,
) -> Result<()> {
    let paths = VaultPaths::new();
    let notes = collect_all_notes(&paths);
    let limit = limit.unwrap_or(5);
    let min_tags = min_tags.unwrap_or(1);

    // Build incoming link map
    let note_names: HashSet<_> = notes.iter().map(|n| n.name.clone()).collect();
    let mut incoming_links: HashMap<String, usize> = HashMap::new();

    for note in &notes {
        for link in note.wikilinks() {
            if note_names.contains(&link) {
                *incoming_links.entry(link).or_insert(0) += 1;
            }
        }
    }

    // Identify orphan notes
    let orphans: Vec<&Note> = notes
        .iter()
        .filter(|n| !incoming_links.contains_key(&n.name))
        .collect();

    if orphans.is_empty() {
        if json {
            let result = ConnectResult {
                dry_run,
                orphan_count: 0,
                connected_count: 0,
                connections: Vec::new(),
            };
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            println!("{}", "✅ No orphan notes found!".green());
        }
        return Ok(());
    }

    let mut connections = Vec::new();
    let mut connected_count = 0;

    // Setup semantic search if needed
    let mut engine = if semantic {
        let vault_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let tools_path = vault_path.join(".opencode/tools");
        let db_path = tools_path.join("data/search.db");

        if db_path.exists() {
            Some(SearchEngine::new(&vault_path, &db_path)?)
        } else {
            if !json {
                println!(
                    "{}",
                    "⚠️  Semantic index not available. Falling back to tag-based.".yellow()
                );
            }
            None
        }
    } else {
        None
    };

    for orphan in &orphans {
        let related = if semantic && engine.is_some() {
            find_related_semantic(orphan, engine.as_mut().unwrap(), limit)?
        } else {
            find_related_by_tags(orphan, &notes, min_tags, limit)
        };

        if related.is_empty() {
            continue;
        }

        let method = if semantic && engine.is_some() {
            "semantic"
        } else {
            "tags"
        };

        let applied = if !dry_run {
            add_related_section(&orphan.path, &related)?
        } else {
            false
        };

        if applied {
            connected_count += 1;
        }

        connections.push(ConnectionDetail {
            orphan: orphan.name.clone(),
            related_notes: related,
            method: method.to_string(),
            applied,
        });
    }

    let result = ConnectResult {
        dry_run,
        orphan_count: orphans.len(),
        connected_count,
        connections,
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        print_report(&result);
    }

    Ok(())
}

fn find_related_by_tags(
    orphan: &Note,
    notes: &[Note],
    min_tags: usize,
    limit: usize,
) -> Vec<String> {
    let orphan_tags: HashSet<_> = orphan.tags().into_iter().collect();

    if orphan_tags.is_empty() {
        return Vec::new();
    }

    let mut related: Vec<(String, usize)> = Vec::new();

    for note in notes {
        if note.name == orphan.name {
            continue;
        }

        let note_tags: HashSet<_> = note.tags().into_iter().collect();
        let shared_count = orphan_tags.intersection(&note_tags).count();

        if shared_count >= min_tags {
            related.push((note.name.clone(), shared_count));
        }
    }

    // Sort by shared tag count (descending)
    related.sort_by(|a, b| b.1.cmp(&a.1));

    related
        .into_iter()
        .take(limit)
        .map(|(name, _)| name)
        .collect()
}

fn find_related_semantic(
    orphan: &Note,
    engine: &mut SearchEngine,
    limit: usize,
) -> Result<Vec<String>> {
    let gist = match orphan.gist() {
        Some(g) if !g.is_empty() => g,
        _ => return Ok(Vec::new()),
    };

    let results = engine.search(gist, limit + 1)?;

    Ok(results
        .into_iter()
        .filter(|r| r.title != orphan.name)
        .take(limit)
        .map(|r| r.title)
        .collect())
}

fn add_related_section(path: &PathBuf, related: &[String]) -> Result<bool> {
    let content = fs::read_to_string(path)?;

    // Generate wikilinks
    let links: Vec<String> = related
        .iter()
        .map(|name| format!("- [[{}]]", name))
        .collect();
    let links_text = links.join("\n");

    let new_content = if content.contains("## Related") {
        // Append to existing Related section
        let parts: Vec<&str> = content.splitn(2, "## Related").collect();
        if parts.len() == 2 {
            // Find the end of Related section (next ## or end of file)
            let after_related = parts[1];
            if let Some(next_section) = after_related.find("\n## ") {
                let (related_content, rest) = after_related.split_at(next_section);
                format!(
                    "{}## Related{}\n{}\n{}",
                    parts[0],
                    related_content.trim_end(),
                    links_text,
                    rest
                )
            } else {
                // No next section, append at end
                format!(
                    "{}## Related{}\n{}\n",
                    parts[0],
                    after_related.trim_end(),
                    links_text
                )
            }
        } else {
            content.clone()
        }
    } else {
        // Add new Related section at end
        format!("{}\n\n## Related\n\n{}\n", content.trim_end(), links_text)
    };

    if new_content != content {
        fs::write(path, new_content)?;
        return Ok(true);
    }

    Ok(false)
}

fn print_report(result: &ConnectResult) {
    println!("{}", "Connect Orphan Notes".bold());
    println!("{}", "=".repeat(60));
    println!();

    if result.dry_run {
        println!("{}", "🔍 DRY RUN MODE - No changes made".yellow().bold());
        println!();
    }

    println!("Orphan notes found: {}", result.orphan_count);
    println!("Notes with connections: {}", result.connections.len());
    println!();

    if result.connections.is_empty() {
        println!(
            "{}",
            "No connections could be made (notes may lack tags or gist).".yellow()
        );
        return;
    }

    println!("{}", "Connections:".cyan());
    for conn in &result.connections {
        let status = if result.dry_run {
            "[WOULD CONNECT]".yellow()
        } else if conn.applied {
            "[CONNECTED]".green()
        } else {
            "[SKIPPED]".dimmed()
        };

        println!(
            "\n  {} {} ({})",
            status,
            conn.orphan.cyan(),
            conn.method.dimmed()
        );

        for related in &conn.related_notes {
            println!("    → [[{}]]", related);
        }
    }

    println!();
    println!("{}", "-".repeat(60));

    if result.dry_run {
        println!("Run with {} to apply connections.", "--execute".cyan());
    } else {
        println!("Connections applied: {}", result.connected_count);
    }
}
