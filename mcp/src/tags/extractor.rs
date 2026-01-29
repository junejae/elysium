//! Tag extractor - automatically build tag database from existing notes
//!
//! Extracts tags from vault notes and generates embeddings from their gists.

use anyhow::Result;
use std::collections::HashMap;

use crate::core::note::Note;

use super::database::TagDatabase;
use super::embedder::TagEmbedder;

/// Extract tags from notes and populate the database
#[allow(dead_code)]
pub fn extract_tags_from_notes(
    notes: &[Note],
    db: &TagDatabase,
    embedder: &TagEmbedder,
    min_usage: usize,
) -> Result<ExtractResult> {
    // Collect all tags and their associated gists
    let mut tag_gists: HashMap<String, Vec<String>> = HashMap::new();

    for note in notes {
        let tags = note.tags();
        let gist = note.gist().unwrap_or(&note.name).to_string();

        for tag in tags {
            tag_gists.entry(tag).or_default().push(gist.clone());
        }
    }

    let mut added = 0;
    let mut skipped = 0;
    let mut updated = 0;

    for (tag_name, gists) in &tag_gists {
        // Skip low-usage tags
        if gists.len() < min_usage {
            skipped += 1;
            continue;
        }

        // Check if tag already exists
        if db.get_tag(tag_name)?.is_some() {
            // Could update embedding here if needed
            updated += 1;
            continue;
        }

        // Generate description from gists (first few)
        let description = generate_description(tag_name, gists);

        // Generate embedding from combined gists
        let combined_text = gists.join(" ");
        let embedding = embedder.embed(&combined_text)?;

        // Add to database
        db.add_tag_with_embedding(tag_name, &description, &embedding)?;
        added += 1;
    }

    Ok(ExtractResult {
        total_tags: tag_gists.len(),
        added,
        skipped,
        updated,
    })
}

/// Generate a description for a tag based on its usage
fn generate_description(tag_name: &str, gists: &[String]) -> String {
    // Use first 3 gists as context
    let sample_gists: Vec<&str> = gists.iter().take(3).map(|s| s.as_str()).collect();

    if sample_gists.is_empty() {
        return format!("Tag: {}", tag_name);
    }

    // Create description from tag name and sample usage
    format!(
        "{}. Used in contexts like: {}",
        tag_name,
        sample_gists
            .join("; ")
            .chars()
            .take(200)
            .collect::<String>()
    )
}

/// Result of tag extraction
#[derive(Debug)]
#[allow(dead_code)]
pub struct ExtractResult {
    pub total_tags: usize,
    pub added: usize,
    pub skipped: usize,
    pub updated: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_description() {
        let gists = vec![
            "GPU memory optimization".to_string(),
            "CUDA programming guide".to_string(),
        ];
        let desc = generate_description("gpu", &gists);
        assert!(desc.contains("gpu"));
        assert!(desc.contains("GPU memory"));
    }
}
