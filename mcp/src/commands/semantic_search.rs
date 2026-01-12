//! Semantic Search command - uses plugin index for search

use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::core::paths::VaultPaths;
use crate::search::engine::simple_search;
use crate::search::PluginSearchEngine;

fn get_vault_path() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

pub fn run(query: &str, limit: Option<usize>, json: bool, fallback: bool) -> Result<()> {
    let vault_path = get_vault_path();
    let limit = limit.unwrap_or(5);

    // Try to use plugin index first
    if !fallback {
        match PluginSearchEngine::load(&vault_path) {
            Ok(engine) => {
                let results = engine.search(query, limit)?;
                return print_results(&results, query, json, false);
            }
            Err(e) => {
                if !json {
                    eprintln!("{} Plugin index not available: {}", "!".yellow(), e);
                    eprintln!("{} Falling back to simple search", "→".dimmed());
                }
            }
        }
    }

    // Fallback to simple search
    run_simple_search(&vault_path, query, limit, json)
}

fn print_results(
    results: &[crate::search::engine::SearchResult],
    query: &str,
    json: bool,
    _is_simple: bool,
) -> Result<()> {
    if json {
        let json_results: Vec<_> = results
            .iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.id,
                    "path": r.path,
                    "title": r.title,
                    "gist": r.gist,
                    "type": r.note_type,
                    "area": r.area,
                    "score": r.score,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json_results)?);
    } else {
        if results.is_empty() {
            println!("{} No results found for: {}", "→".dimmed(), query.cyan());
            return Ok(());
        }

        println!(
            "{} {} results for: {}",
            "→".dimmed(),
            results.len(),
            query.cyan()
        );
        println!();

        for (i, result) in results.iter().enumerate() {
            let score_str = format!("{:.2}", result.score);
            let score_colored = if result.score > 0.8 {
                score_str.green()
            } else if result.score > 0.6 {
                score_str.yellow()
            } else {
                score_str.dimmed()
            };

            println!(
                "{}. [{}] {}",
                (i + 1).to_string().bold(),
                score_colored,
                result.title.cyan()
            );

            if let Some(ref gist) = result.gist {
                // Truncate gist for display (char-aware for Unicode)
                let display_gist = if gist.chars().count() > 100 {
                    format!("{}...", gist.chars().take(100).collect::<String>())
                } else {
                    gist.clone()
                };
                println!("   {}", display_gist.dimmed());
            }

            if let (Some(ref note_type), Some(ref area)) = (&result.note_type, &result.area) {
                println!("   {} | {}", note_type, area);
            }
            println!();
        }
    }

    Ok(())
}

/// Run simple string-based search (fallback)
fn run_simple_search(vault_path: &PathBuf, query: &str, limit: usize, json: bool) -> Result<()> {
    let vault_paths = VaultPaths::from_root(vault_path.clone());
    let results = simple_search(&vault_paths, query, limit);

    if json {
        let json_results: Vec<_> = results
            .iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.id,
                    "path": r.path,
                    "title": r.title,
                    "gist": r.gist,
                    "type": r.note_type,
                    "area": r.area,
                    "score": r.score,
                    "mode": "simple",
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json_results)?);
    } else {
        if !json {
            println!(
                "{} Using simple search (semantic index not available)",
                "!".yellow()
            );
            println!();
        }

        if results.is_empty() {
            println!("{} No results found for: {}", "→".dimmed(), query.cyan());
            return Ok(());
        }

        println!(
            "{} {} results for: {}",
            "→".dimmed(),
            results.len(),
            query.cyan()
        );
        println!();

        for (i, result) in results.iter().enumerate() {
            let score_str = format!("{:.0}%", result.score * 100.0);

            println!(
                "{}. [{}] {}",
                (i + 1).to_string().bold(),
                score_str.dimmed(),
                result.title.cyan()
            );

            if let Some(ref gist) = result.gist {
                // Truncate gist for display (char-aware for Unicode)
                let display_gist = if gist.chars().count() > 100 {
                    format!("{}...", gist.chars().take(100).collect::<String>())
                } else {
                    gist.clone()
                };
                println!("   {}", display_gist.dimmed());
            }
            println!();
        }
    }

    Ok(())
}
