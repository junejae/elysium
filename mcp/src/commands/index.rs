//! Index command - Build semantic search index

use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::search::engine::SearchEngine;

fn get_default_paths() -> (PathBuf, PathBuf) {
    let vault_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let tools_path = vault_path.join(".opencode/tools");
    let db_path = tools_path.join("data/search.db");

    (vault_path, db_path)
}

pub fn run(status_only: bool, rebuild: bool, json: bool) -> Result<()> {
    let (vault_path, db_path) = get_default_paths();

    if status_only {
        return show_status(&db_path, json);
    }

    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if rebuild && db_path.exists() {
        std::fs::remove_file(&db_path)?;
        if !json {
            println!("{} Removed existing index", "→".dimmed());
        }
    }

    let mut engine = SearchEngine::new(&vault_path, &db_path)?;

    if !json {
        println!("{} Building search index...", "→".dimmed());
    }

    // Index all notes
    let stats = engine.index_all()?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "indexed": stats.indexed,
                "skipped": stats.skipped,
                "failed": stats.failed,
                "duration_ms": stats.duration_ms,
            })
        );
    } else {
        println!();
        println!(
            "{} Indexed {} notes in {:.2}s",
            "✓".green().bold(),
            stats.indexed.to_string().cyan(),
            stats.duration_ms as f64 / 1000.0
        );
        if stats.skipped > 0 {
            println!(
                "  {} {} notes skipped (no gist)",
                "→".dimmed(),
                stats.skipped
            );
        }
        if stats.failed > 0 {
            println!("  {} {} notes failed", "✗".red(), stats.failed);
        }
        println!("  {} Index saved to: {}", "→".dimmed(), db_path.display());
    }

    Ok(())
}

/// Show index status
fn show_status(db_path: &PathBuf, json: bool) -> Result<()> {
    if !db_path.exists() {
        if json {
            println!(
                "{}",
                serde_json::json!({
                    "exists": false,
                    "error": "Index not found"
                })
            );
        } else {
            println!(
                "{} Index not found. Run {} first.",
                "!".yellow().bold(),
                "vault index".cyan()
            );
        }
        return Ok(());
    }

    // Open database and get stats (use default HTP dimension for stats query)
    use crate::search::vectordb::VectorDB;
    let db = VectorDB::open(db_path, 384)?;
    let stats = db.get_stats()?;

    // Get file size
    let file_size = std::fs::metadata(db_path).map(|m| m.len()).unwrap_or(0);

    if json {
        println!(
            "{}",
            serde_json::json!({
                "exists": true,
                "note_count": stats.note_count,
                "embedding_count": stats.embedding_count,
                "last_indexed": stats.last_indexed,
                "file_size_bytes": file_size,
            })
        );
    } else {
        println!("{}", "Index Status".bold());
        println!();
        println!(
            "  {} {} notes indexed",
            "→".dimmed(),
            stats.note_count.to_string().cyan()
        );
        println!(
            "  {} {} embeddings",
            "→".dimmed(),
            stats.embedding_count.to_string().cyan()
        );
        println!(
            "  {} Size: {:.2} KB",
            "→".dimmed(),
            file_size as f64 / 1024.0
        );
        if let Some(ts) = stats.last_indexed {
            let dt = chrono::DateTime::from_timestamp(ts, 0)
                .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "Unknown".to_string());
            println!("  {} Last indexed: {}", "→".dimmed(), dt);
        }
    }

    Ok(())
}
