use anyhow::Result;
use colored::*;
use std::path::PathBuf;

use crate::core::note::collect_all_notes;
use crate::core::paths::VaultPaths;
use crate::search::engine::SearchEngine;

pub fn run(
    note_name: &str,
    min_tags: Option<usize>,
    semantic: bool,
    limit: Option<usize>,
    boost_type: bool,
    boost_area: bool,
    json: bool,
) -> Result<()> {
    let paths = VaultPaths::new();
    let notes = collect_all_notes(&paths);

    let target_note = notes.iter().find(|n| n.name == note_name);

    let target_note = match target_note {
        Some(n) => n,
        None => {
            if json {
                println!(r#"{{"error": "Note '{}' not found."}}"#, note_name);
            } else {
                println!("{}", format!("Note '{}' not found.", note_name).red());
            }
            std::process::exit(1);
        }
    };

    if semantic {
        run_semantic(
            target_note,
            limit.unwrap_or(10),
            boost_type,
            boost_area,
            json,
        )
    } else {
        run_tags(target_note, &notes, min_tags, json)
    }
}

fn run_semantic(
    target_note: &crate::core::note::Note,
    limit: usize,
    boost_type: bool,
    boost_area: bool,
    json: bool,
) -> Result<()> {
    use crate::search::engine::BoostOptions;

    let gist = match target_note.gist() {
        Some(g) if !g.is_empty() => g,
        _ => {
            if json {
                println!(r#"{{"error": "Note has no gist for semantic search."}}"#);
            } else {
                println!("{}", "Note has no gist for semantic search.".yellow());
            }
            return Ok(());
        }
    };

    let vault_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let tools_path = vault_path.join(".opencode/tools");
    let db_path = tools_path.join("data/search.db");

    if !db_path.exists() {
        if json {
            println!(r#"{{"error": "Semantic index not available. Run 'elysium index' first."}}"#);
        } else {
            println!(
                "{}",
                "Semantic index not available. Run 'elysium index' first.".yellow()
            );
        }
        return Ok(());
    }

    let mut engine = SearchEngine::new(&vault_path, &db_path)?;

    let results = if boost_type || boost_area {
        let boost = BoostOptions::from_source(
            target_note.note_type(),
            target_note.area(),
            boost_type,
            boost_area,
        );
        engine.search_with_boost(gist, limit + 1, &boost)?
    } else {
        engine.search(gist, limit + 1)?
    };

    let filtered: Vec<_> = results
        .into_iter()
        .filter(|r| r.title != target_note.name)
        .take(limit)
        .collect();

    if json {
        let json_results: Vec<_> = filtered
            .iter()
            .map(|r| {
                serde_json::json!({
                    "title": r.title,
                    "path": r.path,
                    "gist": r.gist,
                    "type": r.note_type,
                    "area": r.area,
                    "score": r.score,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json_results)?);
    } else {
        println!("{}", "Related Notes (Semantic)".bold());
        println!("{}", "=".repeat(60));
        println!("Source: {}", target_note.name.cyan());
        if let Some(ref g) = target_note.gist() {
            let display = if g.chars().count() > 80 {
                format!("{}...", g.chars().take(80).collect::<String>())
            } else {
                g.to_string()
            };
            println!("Gist: {}", display.dimmed());
        }
        println!();

        if filtered.is_empty() {
            println!("{}", "No related notes found.".yellow());
        } else {
            println!("Found {} related notes:", filtered.len());
            println!();

            for (i, result) in filtered.iter().enumerate() {
                let score_pct = format!("{:.0}%", result.score * 100.0);
                let score_colored = if result.score > 0.8 {
                    score_pct.green()
                } else if result.score > 0.6 {
                    score_pct.yellow()
                } else {
                    score_pct.dimmed()
                };

                println!(
                    "{}. [{}] {}",
                    (i + 1).to_string().bold(),
                    score_colored,
                    result.title.cyan()
                );

                if let Some(ref gist) = result.gist {
                    let display = if gist.chars().count() > 80 {
                        format!("{}...", gist.chars().take(80).collect::<String>())
                    } else {
                        gist.clone()
                    };
                    println!("   {}", display.dimmed());
                }
            }
        }
    }

    Ok(())
}

fn run_tags(
    target_note: &crate::core::note::Note,
    notes: &[crate::core::note::Note],
    min_tags: Option<usize>,
    json: bool,
) -> Result<()> {
    let target_tags: std::collections::HashSet<_> = target_note.tags().into_iter().collect();

    if target_tags.is_empty() {
        if json {
            println!(r#"{{"error": "Note has no tags."}}"#);
        } else {
            println!(
                "{}",
                format!("Note '{}' has no tags.", target_note.name).yellow()
            );
        }
        return Ok(());
    }

    let min_shared = min_tags.unwrap_or(1);
    let mut related: Vec<(String, Vec<String>, usize)> = Vec::new();

    for note in notes {
        if note.name == target_note.name {
            continue;
        }

        let note_tags: std::collections::HashSet<_> = note.tags().into_iter().collect();
        let shared: Vec<_> = target_tags.intersection(&note_tags).cloned().collect();

        if shared.len() >= min_shared {
            related.push((note.name.clone(), shared.clone(), shared.len()));
        }
    }

    related.sort_by(|a, b| b.2.cmp(&a.2));

    if json {
        let json_results: Vec<_> = related
            .iter()
            .take(20)
            .map(|(name, tags, count)| {
                serde_json::json!({
                    "title": name,
                    "shared_tags": tags,
                    "shared_count": count,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json_results)?);
    } else {
        println!("{}", "Related Notes (Tags)".bold());
        println!("{}", "=".repeat(60));
        println!("Source: {}", target_note.name.cyan());
        println!("Tags: {:?}", target_tags);
        println!("Minimum shared tags: {}", min_shared);
        println!();

        if related.is_empty() {
            println!("{}", "No related notes found.".yellow());
        } else {
            println!("Found {} related notes:", related.len());
            println!();

            for (name, shared_tags, count) in related.iter().take(20) {
                println!(
                    "  {} ({} shared: {})",
                    name.cyan(),
                    count,
                    shared_tags.join(", ")
                );
            }

            if related.len() > 20 {
                println!();
                println!(
                    "{}",
                    format!("... and {} more", related.len() - 20).dimmed()
                );
            }
        }
    }

    Ok(())
}
