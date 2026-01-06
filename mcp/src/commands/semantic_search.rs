//! Semantic Search command - AI-powered note search

use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::core::paths::VaultPaths;
use crate::search::engine::{simple_search, SearchEngine};

/// Get default paths for search engine
fn get_default_paths() -> (PathBuf, PathBuf, PathBuf) {
    let vault_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let tools_path = vault_path.join(".opencode/tools");
    let db_path = tools_path.join("data/search.db");
    let model_path = tools_path.join("models/model.onnx");

    (vault_path, db_path, model_path)
}

/// Run semantic search command
pub fn run(query: &str, limit: Option<usize>, json: bool, fallback: bool) -> Result<()> {
    let (vault_path, db_path, model_path) = get_default_paths();
    let limit = limit.unwrap_or(5);

    // Check if we should use fallback (simple string search)
    let use_fallback = fallback || !model_path.exists() || !db_path.exists();

    if use_fallback {
        return run_simple_search(&vault_path, query, limit, json);
    }

    // Use semantic search
    let mut engine = SearchEngine::new(&vault_path, &db_path, &model_path)?;
    let results = engine.search(query, limit)?;

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
