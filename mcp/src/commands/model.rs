//! Model management commands - Download and manage Model2Vec models

use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::core::config::Config;
use crate::search::embedder::{Embedder, Model2VecEmbedder};

/// Run model subcommand
pub fn run(subcmd: &str, json: bool) -> Result<()> {
    match subcmd {
        "download" => download(json),
        "status" => status(json),
        _ => {
            if !json {
                println!("{} Unknown subcommand: {}", "!".yellow().bold(), subcmd);
                println!();
                println!("Available subcommands:");
                println!(
                    "  {} - Download Model2Vec model for advanced search",
                    "download".cyan()
                );
                println!("  {} - Show model status", "status".cyan());
            }
            Ok(())
        }
    }
}

/// Download Model2Vec model from HuggingFace Hub
fn download(json: bool) -> Result<()> {
    let vault_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let config = Config::load(&vault_path);

    let model_id = &config.features.advanced_semantic_search.model_id;

    // Check for local model path first
    let local_model_path = vault_path.join(".opencode/tools/models/potion-multilingual-128M");

    if !json {
        if local_model_path.exists() {
            println!("{} Loading model from local path...", "→".dimmed());
        } else {
            println!("{} Downloading model: {}", "→".dimmed(), model_id.cyan());
            println!("  This may take a few minutes on first download...");
            println!();
            println!(
                "  {} If download fails, run this Python command first:",
                "ℹ".blue()
            );
            println!("    python -c \"from model2vec import StaticModel; m = StaticModel.from_pretrained('{}'); m.save_pretrained('.opencode/tools/models/potion-multilingual-128M')\"", model_id);
            println!();
        }
    }

    // Try local path first, then HuggingFace
    let result = if local_model_path.exists() {
        Model2VecEmbedder::from_path(&local_model_path)
    } else {
        Model2VecEmbedder::from_pretrained(model_id)
    };

    match result {
        Ok(embedder) => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "success": true,
                        "model_id": model_id,
                        "dimension": embedder.dimension(),
                        "name": embedder.name(),
                    })
                );
            } else {
                println!("{} Model downloaded successfully!", "✓".green().bold());
                println!();
                println!("  {} Model: {}", "→".dimmed(), model_id);
                println!("  {} Dimension: {}", "→".dimmed(), embedder.dimension());
                println!();
                println!("To enable advanced search, add to your config:");
                println!();
                println!(r#"  "advancedSemanticSearch": {{"#);
                println!(r#"    "enabled": true,"#);
                println!(r#"    "modelDownloaded": true"#);
                println!(r#"  }}"#);
                println!();
                println!("Then rebuild the index:");
                println!("  {} index --rebuild", "elysium".cyan());
            }

            // Update config to mark model as downloaded
            let mut updated_config = config.clone();
            updated_config
                .features
                .advanced_semantic_search
                .model_downloaded = true;
            // Save local path if model was loaded from local
            if local_model_path.exists() {
                updated_config.features.advanced_semantic_search.model_path =
                    Some(".opencode/tools/models/potion-multilingual-128M".to_string());
            }
            if let Err(e) = updated_config.save(&vault_path) {
                if !json {
                    eprintln!("{} Could not update config: {}", "!".yellow().bold(), e);
                }
            }

            Ok(())
        }
        Err(e) => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "success": false,
                        "error": e.to_string(),
                    })
                );
            } else {
                println!("{} Failed to download model: {}", "✗".red().bold(), e);
            }
            Err(e)
        }
    }
}

/// Show model status
fn status(json: bool) -> Result<()> {
    let vault_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let config = Config::load(&vault_path);

    let adv_config = &config.features.advanced_semantic_search;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "enabled": adv_config.enabled,
                "model_downloaded": adv_config.model_downloaded,
                "model_id": adv_config.model_id,
                "model_path": adv_config.model_path,
                "ready": config.features.is_advanced_search_ready(),
            })
        );
    } else {
        println!("{}", "Model Status".bold());
        println!();

        // Model ID
        println!(
            "  {} Model ID: {}",
            "→".dimmed(),
            adv_config.model_id.cyan()
        );

        // Enabled status
        let enabled_status = if adv_config.enabled {
            "Enabled".green()
        } else {
            "Disabled".yellow()
        };
        println!("  {} Advanced Search: {}", "→".dimmed(), enabled_status);

        // Downloaded status
        let downloaded_status = if adv_config.model_downloaded {
            "Downloaded".green()
        } else {
            "Not Downloaded".yellow()
        };
        println!("  {} Model Status: {}", "→".dimmed(), downloaded_status);

        // Ready status
        if config.features.is_advanced_search_ready() {
            println!();
            println!(
                "  {} Advanced semantic search is {}",
                "✓".green().bold(),
                "ready".green().bold()
            );
        } else {
            println!();
            if !adv_config.model_downloaded {
                println!(
                    "  {} Run {} to download the model",
                    "!".yellow().bold(),
                    "elysium model download".cyan()
                );
            }
            if !adv_config.enabled {
                println!(
                    "  {} Set {} in config to enable",
                    "!".yellow().bold(),
                    "advancedSemanticSearch.enabled: true".cyan()
                );
            }
        }

        // Show HuggingFace cache location hint
        println!();
        println!(
            "  {} HuggingFace cache: ~/.cache/huggingface/hub/",
            "ℹ".blue()
        );
    }

    Ok(())
}
