use anyhow::Result;
use colored::*;

use crate::core::note::collect_all_notes;
use crate::core::paths::VaultPaths;

pub fn run(note_name: &str, min_tags: Option<usize>) -> Result<()> {
    let paths = VaultPaths::new();
    let notes = collect_all_notes(&paths);

    let target_note = notes.iter().find(|n| n.name == note_name);

    let target_note = match target_note {
        Some(n) => n,
        None => {
            println!("{}", format!("Note '{}' not found.", note_name).red());
            std::process::exit(1);
        }
    };

    let target_tags: std::collections::HashSet<_> = target_note.tags().into_iter().collect();

    if target_tags.is_empty() {
        println!("{}", format!("Note '{}' has no tags.", note_name).yellow());
        return Ok(());
    }

    let min_shared = min_tags.unwrap_or(1);
    let mut related: Vec<(String, Vec<String>, usize)> = Vec::new();

    for note in &notes {
        if note.name == note_name {
            continue;
        }

        let note_tags: std::collections::HashSet<_> = note.tags().into_iter().collect();
        let shared: Vec<_> = target_tags.intersection(&note_tags).cloned().collect();

        if shared.len() >= min_shared {
            related.push((note.name.clone(), shared.clone(), shared.len()));
        }
    }

    related.sort_by(|a, b| b.2.cmp(&a.2));

    println!("{}", "Related Notes".bold());
    println!("{}", "=".repeat(60));
    println!("Source: {}", note_name.cyan());
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

    Ok(())
}
