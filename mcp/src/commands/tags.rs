use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use colored::*;
use serde::Serialize;

use crate::core::note::collect_all_notes;
use crate::core::paths::VaultPaths;
use crate::tags::keyword::KeywordExtractor;
use crate::tags::{extract_tags_from_notes, seed_database, TagDatabase, TagEmbedder, TagMatcher};

#[derive(Serialize)]
struct TagsResult {
    total_notes: usize,
    total_tags: usize,
    unique_tags: usize,
    notes_without_tags: usize,
    tag_usage: Vec<TagUsage>,
    low_usage_tags: Vec<String>,
    suggestions: Vec<Suggestion>,
}

#[derive(Serialize)]
struct TagUsage {
    tag: String,
    count: usize,
    notes: Vec<String>,
}

#[derive(Serialize)]
struct Suggestion {
    action: String,
    tag: String,
    reason: String,
}

pub fn run(analyze: bool, json: bool) -> Result<()> {
    let paths = VaultPaths::new();
    let notes = collect_all_notes(&paths);

    let mut tag_notes: HashMap<String, Vec<String>> = HashMap::new();
    let mut notes_without_tags = 0;
    let mut total_tags = 0;

    for note in &notes {
        let tags = note.tags();
        if tags.is_empty() {
            notes_without_tags += 1;
        }
        total_tags += tags.len();

        for tag in tags {
            tag_notes.entry(tag).or_default().push(note.name.clone());
        }
    }

    let mut tag_usage: Vec<TagUsage> = tag_notes
        .into_iter()
        .map(|(tag, notes)| TagUsage {
            tag,
            count: notes.len(),
            notes,
        })
        .collect();

    tag_usage.sort_by(|a, b| b.count.cmp(&a.count));

    let low_usage_tags: Vec<String> = tag_usage
        .iter()
        .filter(|t| t.count <= 2)
        .map(|t| t.tag.clone())
        .collect();

    let mut suggestions = Vec::new();

    if analyze {
        // Find similar tags that might be mergeable
        let tag_names: Vec<&str> = tag_usage.iter().map(|t| t.tag.as_str()).collect();
        for t in &tag_names {
            // Check for potential duplicates (very similar names)
            for other in &tag_names {
                if t != other {
                    let t_lower = t.to_lowercase();
                    let other_lower = other.to_lowercase();

                    // Check if one is prefix of another
                    if t_lower.starts_with(&other_lower) || other_lower.starts_with(&t_lower) {
                        if !suggestions.iter().any(|s: &Suggestion| {
                            (s.tag == *t || s.tag == *other) && s.action == "merge"
                        }) {
                            suggestions.push(Suggestion {
                                action: "merge".to_string(),
                                tag: format!("{} / {}", t, other),
                                reason: "Similar tag names - consider merging".to_string(),
                            });
                        }
                    }
                }
            }
        }

        // Suggest removing very low usage tags
        for tag in &low_usage_tags {
            let usage = tag_usage.iter().find(|t| &t.tag == tag);
            if let Some(u) = usage {
                if u.count == 1 {
                    suggestions.push(Suggestion {
                        action: "review".to_string(),
                        tag: tag.clone(),
                        reason: format!("Used only once in: {}", u.notes.join(", ")),
                    });
                }
            }
        }
    }

    let result = TagsResult {
        total_notes: notes.len(),
        total_tags,
        unique_tags: tag_usage.len(),
        notes_without_tags,
        tag_usage,
        low_usage_tags,
        suggestions,
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        print_report(&result, analyze);
    }

    Ok(())
}

fn print_report(result: &TagsResult, analyze: bool) {
    println!("{}", "Vault Tag Analysis".bold());
    println!("{}", "=".repeat(60));
    println!();
    println!("Total notes: {}", result.total_notes);
    println!("Notes without tags: {}", result.notes_without_tags);
    println!("Total tag usages: {}", result.total_tags);
    println!("Unique tags: {}", result.unique_tags);
    println!("Low usage tags (≤2): {}", result.low_usage_tags.len());
    println!();

    println!("{}", "Tag Usage (sorted by count):".cyan().bold());
    println!("{}", "-".repeat(60));

    for usage in &result.tag_usage {
        let count_str = format!("{:>3}", usage.count);
        let count_colored = if usage.count >= 5 {
            count_str.green()
        } else if usage.count >= 2 {
            count_str.yellow()
        } else {
            count_str.red()
        };
        println!("  {} × {}", count_colored, usage.tag);
    }

    if analyze && !result.suggestions.is_empty() {
        println!();
        println!("{}", "Suggestions:".yellow().bold());
        println!("{}", "-".repeat(60));

        for suggestion in &result.suggestions {
            let action = match suggestion.action.as_str() {
                "merge" => "🔀 MERGE".cyan(),
                "review" => "🔍 REVIEW".yellow(),
                _ => "📝 NOTE".normal(),
            };
            println!("  {} [{}]", action, suggestion.tag);
            println!("     {}", suggestion.reason);
        }
    }

    println!();
    println!("{}", "=".repeat(60));

    if result.low_usage_tags.len() > result.unique_tags / 2 {
        println!(
            "{}",
            "⚠️  Warning: Many low-usage tags detected. Consider cleanup.".yellow()
        );
    }
}

// ===== New tag automation functions =====

/// Get tag database path
fn get_tag_db_path() -> PathBuf {
    let paths = VaultPaths::new();
    paths.root.join(".opencode/tools/data/tags.db")
}

/// Initialize tag database with seed data
pub fn run_init(force: bool) -> Result<()> {
    let db_path = get_tag_db_path();

    // Create parent directories if needed
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Check if already initialized
    if db_path.exists() && !force {
        println!(
            "{}",
            "Tag database already exists. Use --force to reinitialize.".yellow()
        );
        return Ok(());
    }

    // Remove existing if force
    if force && db_path.exists() {
        std::fs::remove_file(&db_path)?;
    }

    println!("{}", "Initializing tag database...".cyan());
    println!("Database path: {}", db_path.display());
    println!();

    // Load Model2Vec (this downloads the model on first run)
    println!(
        "{}",
        "Loading Model2Vec (potion-multilingual-128M)...".dimmed()
    );
    println!("{}", "This may take a moment on first run...".dimmed());

    let embedder = TagEmbedder::default_multilingual().context("Failed to load Model2Vec model")?;

    println!("{}", "Model loaded successfully!".green());
    println!();

    // Open database
    let db = TagDatabase::open(&db_path)?;

    // Seed with initial tags
    println!("{}", "Seeding database with core tags...".cyan());

    let count = seed_database(&db, &embedder)?;

    println!();
    println!("{}", "=".repeat(50));
    println!(
        "{} {} tags initialized",
        "✓".green(),
        count.to_string().bold()
    );
    println!("{}", "=".repeat(50));

    Ok(())
}

/// Suggest tags for given text
pub fn run_suggest(text: &str, limit: usize, discover: bool, json: bool) -> Result<()> {
    let db_path = get_tag_db_path();

    if !db_path.exists() {
        eprintln!(
            "{}",
            "Tag database not initialized. Run 'elysium tags init' first.".red()
        );
        std::process::exit(1);
    }

    let embedder = TagEmbedder::default_multilingual().context("Failed to load Model2Vec model")?;
    let db = TagDatabase::open(&db_path)?;
    let matcher = TagMatcher::new(embedder, db);

    // Load keyword extractor if discovery mode is enabled
    let keyword_extractor = if discover {
        if !json {
            println!(
                "{}",
                "Loading keyword extractor for discovery mode...".dimmed()
            );
        }
        Some(KeywordExtractor::from_default_cache().context("Failed to load KeywordExtractor")?)
    } else {
        None
    };

    let suggestions =
        matcher.suggest_tags_with_discovery(text, limit, keyword_extractor.as_ref())?;

    if json {
        println!("{}", serde_json::to_string_pretty(&suggestions)?);
    } else {
        let mode = if discover {
            "Tag Suggestions (Discovery Mode)"
        } else {
            "Tag Suggestions"
        };
        println!("{}", mode.bold());
        println!("{}", "=".repeat(50));
        println!("Input: {}", text.dimmed());
        println!();

        if suggestions.is_empty() {
            println!("{}", "No matching tags found.".yellow());
        } else {
            for (i, s) in suggestions.iter().enumerate() {
                let score_pct = format!("{:.0}%", s.score * 100.0);
                let score_colored = if s.score >= 0.8 {
                    score_pct.green()
                } else if s.score >= 0.5 {
                    score_pct.yellow()
                } else {
                    score_pct.red()
                };

                // Highlight discovered vs DB-matched tags
                let tag_display = if s.reason.starts_with("Discovered") {
                    format!("{} (NEW)", s.tag).magenta().bold()
                } else {
                    s.tag.cyan().bold()
                };

                println!(
                    "  {}. {} [{}] - {}",
                    i + 1,
                    tag_display,
                    score_colored,
                    s.reason.dimmed()
                );
            }
        }
    }

    Ok(())
}

/// Sync tags for all notes
pub fn run_sync(execute: bool, discover: bool, json: bool) -> Result<()> {
    let db_path = get_tag_db_path();

    if !db_path.exists() {
        eprintln!(
            "{}",
            "Tag database not initialized. Run 'elysium tags init' first.".red()
        );
        std::process::exit(1);
    }

    let paths = VaultPaths::new();
    let notes = collect_all_notes(&paths);

    let embedder = TagEmbedder::default_multilingual().context("Failed to load Model2Vec model")?;
    let db = TagDatabase::open(&db_path)?;
    let matcher = TagMatcher::new(embedder, db);

    // Load keyword extractor if discovery mode is enabled
    let keyword_extractor = if discover {
        if !json {
            println!(
                "{}",
                "Loading keyword extractor for discovery mode...".dimmed()
            );
        }
        Some(KeywordExtractor::from_default_cache().context("Failed to load KeywordExtractor")?)
    } else {
        None
    };

    #[derive(Serialize)]
    struct SyncResult {
        note: String,
        current_tags: Vec<String>,
        suggested_tags: Vec<String>,
        action: String,
    }

    let mut results = Vec::new();

    for note in &notes {
        // Use gist if available, otherwise title
        let search_text = note.gist().unwrap_or(&note.name);

        let suggestions =
            matcher.suggest_tags_with_discovery(search_text, 5, keyword_extractor.as_ref())?;
        let suggested_tags: Vec<String> = suggestions.iter().map(|s| s.tag.clone()).collect();
        let current_tags = note.tags();

        // Determine action
        let action = if current_tags.is_empty() && !suggested_tags.is_empty() {
            "add"
        } else if current_tags != suggested_tags {
            "update"
        } else {
            "skip"
        };

        if action != "skip" {
            results.push(SyncResult {
                note: note.name.clone(),
                current_tags,
                suggested_tags,
                action: action.to_string(),
            });
        }
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&results)?);
    } else {
        let mode = if discover {
            "Tag Sync Preview (Discovery Mode)"
        } else {
            "Tag Sync Preview"
        };
        println!("{}", mode.bold());
        println!("{}", "=".repeat(60));
        println!();

        if results.is_empty() {
            println!(
                "{}",
                "All notes have appropriate tags. Nothing to sync.".green()
            );
        } else {
            for r in &results {
                let action_colored = match r.action.as_str() {
                    "add" => "ADD".green(),
                    "update" => "UPDATE".yellow(),
                    _ => "SKIP".dimmed(),
                };

                println!("[{}] {}", action_colored, r.note.bold());
                println!(
                    "  Current:   {}",
                    if r.current_tags.is_empty() {
                        "(none)".dimmed().to_string()
                    } else {
                        r.current_tags.join(", ")
                    }
                );
                println!("  Suggested: {}", r.suggested_tags.join(", ").cyan());
                println!();
            }

            println!("{}", "=".repeat(60));
            println!(
                "Total: {} notes to update",
                results.len().to_string().bold()
            );

            if !execute {
                println!();
                println!("{}", "Dry run. Use --execute to apply changes.".yellow());
            }
        }
    }

    // Apply changes when execute is true
    if execute && !results.is_empty() {
        println!();
        println!("{}", "Applying changes...".cyan());

        let mut success_count = 0;
        let mut error_count = 0;

        for r in &results {
            // Find the note again to get its path
            let note = notes.iter().find(|n| n.name == r.note);
            if let Some(note) = note {
                match update_note_tags(&note.path, &r.suggested_tags) {
                    Ok(_) => {
                        success_count += 1;
                        if !json {
                            println!("  {} {}", "✓".green(), r.note);
                        }
                    }
                    Err(e) => {
                        error_count += 1;
                        if !json {
                            println!("  {} {} - {}", "✗".red(), r.note, e);
                        }
                    }
                }
            }
        }

        if !json {
            println!();
            println!("{}", "=".repeat(60));
            println!(
                "Applied: {} success, {} errors",
                success_count.to_string().green(),
                error_count.to_string().red()
            );
        }
    }

    Ok(())
}

/// Update tags in a note's frontmatter
fn update_note_tags(path: &std::path::Path, new_tags: &[String]) -> Result<()> {
    let content = std::fs::read_to_string(path)?;

    // Check if file has frontmatter
    if !content.starts_with("---") {
        anyhow::bail!("Note has no frontmatter");
    }

    // Find frontmatter boundaries
    let end_idx = content[3..]
        .find("---")
        .map(|i| i + 3)
        .ok_or_else(|| anyhow::anyhow!("Invalid frontmatter"))?;

    let frontmatter = &content[..end_idx + 3];
    let body = &content[end_idx + 3..];

    // Parse and update frontmatter
    let new_frontmatter = update_tags_in_frontmatter(frontmatter, new_tags);

    // Write back
    let new_content = format!("{}{}", new_frontmatter, body);
    std::fs::write(path, new_content)?;

    Ok(())
}

/// Update or add elysium_tags in frontmatter string
fn update_tags_in_frontmatter(frontmatter: &str, new_tags: &[String]) -> String {
    let lines: Vec<&str> = frontmatter.lines().collect();
    let mut result = Vec::new();
    let mut tags_found = false;

    for line in &lines {
        if line.starts_with("elysium_tags:") {
            // Replace existing tags
            result.push(format!("elysium_tags: [{}]", new_tags.join(", ")));
            tags_found = true;
        } else if *line == "---" && result.len() > 1 && !tags_found {
            // Add tags before closing ---
            result.push(format!("elysium_tags: [{}]", new_tags.join(", ")));
            result.push(line.to_string());
            tags_found = true;
        } else {
            result.push(line.to_string());
        }
    }

    // If no closing --- found and no tags added, add them
    if !tags_found && !new_tags.is_empty() {
        // Insert before last line (which should be ---)
        let last = result.pop();
        result.push(format!("elysium_tags: [{}]", new_tags.join(", ")));
        if let Some(l) = last {
            result.push(l);
        }
    }

    result.join("\n")
}

/// Extract tags from existing notes and populate the database
pub fn run_extract(min_usage: usize, json: bool) -> Result<()> {
    let db_path = get_tag_db_path();

    // Create parent directories if needed
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let paths = VaultPaths::new();
    let notes = collect_all_notes(&paths);

    if !json {
        println!("{}", "Extracting tags from vault notes...".cyan());
        println!("Database path: {}", db_path.display());
        println!("Notes to analyze: {}", notes.len());
        println!("Minimum usage: {}", min_usage);
        println!();
    }

    // Load Model2Vec
    if !json {
        println!("{}", "Loading Model2Vec...".dimmed());
    }

    let embedder = TagEmbedder::default_multilingual().context("Failed to load Model2Vec model")?;

    if !json {
        println!("{}", "Model loaded!".green());
        println!();
    }

    // Open or create database
    let db = TagDatabase::open(&db_path)?;

    // Extract tags
    if !json {
        println!("{}", "Extracting and embedding tags...".cyan());
    }

    let result = extract_tags_from_notes(&notes, &db, &embedder, min_usage)?;

    if json {
        #[derive(Serialize)]
        struct ExtractResultJson {
            total_tags: usize,
            added: usize,
            skipped: usize,
            updated: usize,
        }

        let output = ExtractResultJson {
            total_tags: result.total_tags,
            added: result.added,
            skipped: result.skipped,
            updated: result.updated,
        };
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!();
        println!("{}", "=".repeat(50));
        println!("{}", "Extraction Complete".bold());
        println!("{}", "=".repeat(50));
        println!("Total unique tags found: {}", result.total_tags);
        println!("  {} Added to database", result.added.to_string().green());
        println!(
            "  {} Skipped (usage < {})",
            result.skipped.to_string().yellow(),
            min_usage
        );
        println!("  {} Already existed", result.updated.to_string().dimmed());
        println!();

        // Show total tags in DB
        let total_in_db = db.tag_count()?;
        println!("Total tags in database: {}", total_in_db.to_string().bold());
    }

    Ok(())
}

/// Extract keywords from text using Model2Vec token embeddings
pub fn run_keywords(text: &str, limit: usize, json: bool) -> Result<()> {
    if !json {
        println!("{}", "Extracting keywords from text...".cyan());
        println!("{}", "Loading Model2Vec tokenizer...".dimmed());
    }

    let extractor = KeywordExtractor::from_default_cache()
        .context("Failed to load Model2Vec. Make sure potion-multilingual-128M is cached.")?;

    let keywords = extractor.extract_keywords(text, limit)?;

    if json {
        #[derive(Serialize)]
        struct KeywordResult {
            token: String,
            score: f32,
        }

        let output: Vec<KeywordResult> = keywords
            .iter()
            .map(|k| KeywordResult {
                token: k.token.clone(),
                score: k.score,
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!();
        println!("{}", "Extracted Keywords".bold());
        println!("{}", "=".repeat(50));
        println!("Input: {}", text.dimmed());
        println!();

        if keywords.is_empty() {
            println!("{}", "No keywords extracted.".yellow());
        } else {
            for (i, k) in keywords.iter().enumerate() {
                let score_pct = format!("{:.1}%", k.score * 100.0);
                let score_colored = if k.score >= 0.5 {
                    score_pct.green()
                } else if k.score >= 0.3 {
                    score_pct.yellow()
                } else {
                    score_pct.dimmed()
                };

                println!("  {}. {} [{}]", i + 1, k.token.cyan().bold(), score_colored);
            }
        }
    }

    Ok(())
}
