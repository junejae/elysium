//! Search Engine - combines embedding model and vector database
//!
//! Phase 1: gist-based semantic search

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use super::embedding::EmbeddingModel;
use super::vectordb::{IndexStats, NoteRecord, VectorDB};
use crate::core::note::{collect_all_notes, Note};
use crate::core::paths::VaultPaths;
use std::path::PathBuf as StdPathBuf;

/// Search result with note metadata and similarity score
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub id: String,
    pub path: String,
    pub title: String,
    pub gist: Option<String>,
    pub note_type: Option<String>,
    pub area: Option<String>,
    pub score: f32,
}

impl From<(NoteRecord, f32)> for SearchResult {
    fn from((record, score): (NoteRecord, f32)) -> Self {
        Self {
            id: record.id,
            path: record.path,
            title: record.title,
            gist: record.gist,
            note_type: record.note_type,
            area: record.area,
            score,
        }
    }
}

/// Indexing statistics
#[derive(Debug)]
pub struct IndexingStats {
    pub indexed: usize,
    pub skipped: usize,
    pub failed: usize,
    pub duration_ms: u128,
}

/// Search engine combining embedding model and vector database
pub struct SearchEngine {
    model: Option<EmbeddingModel>,
    db: VectorDB,
    vault_paths: VaultPaths,
    model_path: PathBuf,
}

impl SearchEngine {
    /// Create new search engine
    ///
    /// Note: Model is loaded lazily on first search/index operation
    pub fn new(vault_path: &Path, db_path: &Path, model_path: &Path) -> Result<Self> {
        let vault_paths = VaultPaths::from_root(vault_path.to_path_buf());
        let db = VectorDB::open(db_path)?;

        Ok(Self {
            model: None,
            db,
            vault_paths,
            model_path: model_path.to_path_buf(),
        })
    }

    /// Create with in-memory database (for testing)
    pub fn new_in_memory(vault_path: &Path, model_path: &Path) -> Result<Self> {
        let vault_paths = VaultPaths::from_root(vault_path.to_path_buf());
        let db = VectorDB::open_in_memory()?;

        Ok(Self {
            model: None,
            db,
            vault_paths,
            model_path: model_path.to_path_buf(),
        })
    }

    /// Ensure model is loaded
    fn ensure_model(&mut self) -> Result<&EmbeddingModel> {
        if self.model.is_none() {
            let model =
                EmbeddingModel::load(&self.model_path).context("Failed to load embedding model")?;
            self.model = Some(model);
        }
        Ok(self.model.as_ref().unwrap())
    }

    /// Search for notes similar to query
    pub fn search(&mut self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let model = self.ensure_model()?;

        // Generate query embedding
        let query_embedding = model.embed(query)?;

        // Search in vector database
        let results = self.db.search(&query_embedding, limit)?;

        // Convert to SearchResult
        Ok(results.into_iter().map(SearchResult::from).collect())
    }

    /// Index all notes in vault
    pub fn index_all(&mut self) -> Result<IndexingStats> {
        let start = std::time::Instant::now();

        // Collect all notes
        let notes = collect_all_notes(&self.vault_paths);

        let mut indexed = 0;
        let mut skipped = 0;
        let mut failed = 0;

        for note in notes {
            match self.index_note(&note) {
                Ok(true) => indexed += 1,
                Ok(false) => skipped += 1,
                Err(e) => {
                    eprintln!("Failed to index {}: {}", note.name, e);
                    failed += 1;
                }
            }
        }

        let duration_ms = start.elapsed().as_millis();

        // Update metadata
        self.db.set_meta("indexed_count", &indexed.to_string())?;
        self.db.set_meta(
            "last_full_index",
            &chrono::Utc::now().timestamp().to_string(),
        )?;

        Ok(IndexingStats {
            indexed,
            skipped,
            failed,
            duration_ms,
        })
    }

    /// Index a single note
    ///
    /// Returns Ok(true) if indexed, Ok(false) if skipped (no gist)
    pub fn index_note(&mut self, note: &Note) -> Result<bool> {
        // Skip notes without gist
        let gist = match note.gist() {
            Some(g) if !g.is_empty() => g,
            _ => return Ok(false),
        };

        // Ensure model is loaded
        let model = self.ensure_model()?;

        // Generate embedding from gist
        let embedding = model.embed(gist)?;

        // Create note record
        let record = NoteRecord {
            id: note.name.clone(),
            path: note.path.to_string_lossy().to_string(),
            title: note.name.clone(),
            gist: Some(gist.to_string()),
            note_type: note.note_type().map(String::from),
            status: note.status().map(String::from),
            area: note.area().map(String::from),
            tags: note.tags(),
            mtime: note.modified.timestamp(),
        };

        // Upsert to database
        self.db.upsert_note(&record, &embedding)?;

        Ok(true)
    }

    /// Get index statistics
    pub fn get_stats(&self) -> Result<IndexStats> {
        self.db.get_stats()
    }

    /// Check if model is available
    pub fn model_exists(&self) -> bool {
        self.model_path.exists()
    }

    /// Get database path
    pub fn db_path(&self) -> &Path {
        // We can't easily get this from rusqlite Connection
        // This is a limitation - caller should track this
        Path::new("")
    }
}

/// Simple search without ONNX model (for testing or fallback)
/// Uses basic string matching on gist
pub fn simple_search(vault_paths: &VaultPaths, query: &str, limit: usize) -> Vec<SearchResult> {
    let notes = collect_all_notes(vault_paths);
    let query_lower = query.to_lowercase();

    let mut results: Vec<SearchResult> = notes
        .iter()
        .filter_map(|note| {
            let gist = note.gist()?;
            let gist_lower = gist.to_lowercase();

            // Simple relevance score based on query term matches
            let query_terms: Vec<&str> = query_lower.split_whitespace().collect();
            let matched_terms = query_terms
                .iter()
                .filter(|term| gist_lower.contains(*term))
                .count();

            if matched_terms == 0 {
                return None;
            }

            let score = matched_terms as f32 / query_terms.len() as f32;

            Some(SearchResult {
                id: note.name.clone(),
                path: note.path.to_string_lossy().to_string(),
                title: note.name.clone(),
                gist: Some(gist.to_string()),
                note_type: note.note_type().map(String::from),
                area: note.area().map(String::from),
                score,
            })
        })
        .collect();

    // Sort by score descending
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(limit);

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_search() {
        // This test requires actual vault files
        // Just verify the function compiles and returns expected type
        let vault_paths = VaultPaths::from_root(std::path::PathBuf::from("/tmp/nonexistent"));
        let results = simple_search(&vault_paths, "test query", 5);
        assert!(results.is_empty()); // No files in nonexistent path
    }
}
