use std::collections::HashMap;

use anyhow::Result;
use colored::*;
use serde::Serialize;

use crate::core::note::collect_all_notes;
use crate::core::paths::VaultPaths;

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
