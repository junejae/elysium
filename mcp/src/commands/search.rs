use anyhow::Result;
use colored::*;
use regex::RegexBuilder;

use crate::core::note::collect_all_notes;
use crate::core::paths::VaultPaths;

pub fn run(query: &str, gist_only: bool, limit: Option<usize>) -> Result<()> {
    let paths = VaultPaths::new();
    let notes = collect_all_notes(&paths);

    let re = RegexBuilder::new(&regex::escape(query))
        .case_insensitive(true)
        .build()?;

    let mut results = Vec::new();

    for note in &notes {
        let mut matched = false;
        let mut match_context = String::new();

        if re.is_match(&note.name) {
            matched = true;
            match_context = format!("Title: {}", note.name);
        }

        if gist_only {
            if let Some(gist) = note.gist() {
                if re.is_match(gist) {
                    matched = true;
                    match_context = format!("Gist: {}", truncate(gist, 80));
                }
            }
        } else {
            if let Some(gist) = note.gist() {
                if re.is_match(gist) {
                    matched = true;
                    match_context = format!("Gist: {}", truncate(gist, 80));
                }
            }

            if !matched {
                if let Some(mat) = re.find(&note.content) {
                    matched = true;
                    let context = extract_context(&note.content, mat.start(), mat.end(), 30);
                    match_context = format!("Content: ...{}...", context.replace('\n', " "));
                }
            }
        }

        if matched {
            results.push((note.name.clone(), match_context));
        }
    }

    let total = results.len();
    let display_limit = limit.unwrap_or(20);
    let results_to_show = &results[..results.len().min(display_limit)];

    println!("{}", "Search Results".bold());
    println!("{}", "=".repeat(60));
    println!("Query: \"{}\"", query);
    println!("Found: {} matches", total);
    println!();

    if results_to_show.is_empty() {
        println!("{}", "No matches found.".yellow());
    } else {
        for (name, context) in results_to_show {
            println!("{}", name.cyan());
            println!("  {}", context.dimmed());
            println!();
        }

        if total > display_limit {
            println!(
                "{}",
                format!("... and {} more results", total - display_limit).dimmed()
            );
        }
    }

    Ok(())
}

fn truncate(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else {
        format!("{}...", chars[..max_chars].iter().collect::<String>())
    }
}

fn extract_context(
    content: &str,
    match_start: usize,
    match_end: usize,
    context_chars: usize,
) -> String {
    let chars: Vec<char> = content.chars().collect();
    let byte_to_char: std::collections::HashMap<usize, usize> = content
        .char_indices()
        .enumerate()
        .map(|(i, (byte_idx, _))| (byte_idx, i))
        .collect();

    let char_start = byte_to_char.get(&match_start).copied().unwrap_or(0);
    let char_end = byte_to_char.get(&match_end).copied().unwrap_or(chars.len());

    let start = char_start.saturating_sub(context_chars);
    let end = (char_end + context_chars).min(chars.len());

    chars[start..end].iter().collect()
}
