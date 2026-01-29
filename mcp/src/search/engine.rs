//! Search Engine - combines embedding model and vector database
//!
//! Phase 1: gist-based semantic search

use anyhow::Result;
use std::path::Path;

use super::embedder::{create_embedder, Embedder, SearchConfig};
use super::vectordb::{IndexStats, NoteRecord, VectorDB};
use crate::core::note::{collect_all_notes, Note};
use crate::core::paths::VaultPaths;

/// Search result with note metadata and similarity score
#[derive(Debug, Clone)]
pub struct SearchResult {
    #[allow(dead_code)]
    pub id: String,
    pub path: String,
    pub title: String,
    pub gist: Option<String>,
    pub note_type: Option<String>,
    pub area: Option<String>,
    pub score: f32,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct BoostOptions {
    pub boost_type: bool,
    pub boost_area: bool,
    pub source_type: Option<String>,
    pub source_area: Option<String>,
}

#[allow(dead_code)]
impl BoostOptions {
    pub fn from_source(
        note_type: Option<&str>,
        area: Option<&str>,
        boost_type: bool,
        boost_area: bool,
    ) -> Self {
        Self {
            boost_type,
            boost_area,
            source_type: note_type.map(String::from),
            source_area: area.map(String::from),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.boost_type || self.boost_area
    }
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
#[allow(dead_code)]
#[derive(Debug)]
pub struct IndexingStats {
    pub indexed: usize,
    pub skipped: usize,
    pub failed: usize,
    pub duration_ms: u128,
}

/// Search engine combining embedding model and vector database
pub struct SearchEngine {
    embedder: Box<dyn Embedder>,
    db: VectorDB,
    #[allow(dead_code)]
    vault_paths: VaultPaths,
}

impl SearchEngine {
    /// Create new search engine with default (HTP) embedding
    #[allow(dead_code)]
    pub fn new(vault_path: &Path, db_path: &Path) -> Result<Self> {
        Self::with_config(vault_path, db_path, SearchConfig::default())
    }

    /// Create new search engine with specified configuration
    pub fn with_config(vault_path: &Path, db_path: &Path, config: SearchConfig) -> Result<Self> {
        let vault_paths = VaultPaths::from_root(vault_path.to_path_buf());
        let embedder = create_embedder(&config)?;
        let db = VectorDB::open(db_path, embedder.dimension())?;

        Ok(Self {
            embedder,
            db,
            vault_paths,
        })
    }

    /// Create with in-memory database (for testing)
    #[allow(dead_code)]
    pub fn new_in_memory(vault_path: &Path) -> Result<Self> {
        let vault_paths = VaultPaths::from_root(vault_path.to_path_buf());
        let embedder = create_embedder(&SearchConfig::default())?;
        let db = VectorDB::open_in_memory_with_dim(embedder.dimension())?;

        Ok(Self {
            embedder,
            db,
            vault_paths,
        })
    }

    /// Get current embedder name
    #[allow(dead_code)]
    pub fn embedder_name(&self) -> &str {
        self.embedder.name()
    }

    /// Get embedding dimension
    #[allow(dead_code)]
    pub fn embedding_dimension(&self) -> usize {
        self.embedder.dimension()
    }

    pub fn search(&mut self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let query_embedding = self.embedder.embed(query)?;
        let results = self.db.search(&query_embedding, limit)?;
        Ok(results.into_iter().map(SearchResult::from).collect())
    }

    #[allow(dead_code)]
    pub fn search_with_boost(
        &mut self,
        query: &str,
        limit: usize,
        boost: &BoostOptions,
    ) -> Result<Vec<SearchResult>> {
        if !boost.is_enabled() {
            return self.search(query, limit);
        }

        let query_embedding = self.embedder.embed(query)?;
        let raw_results = self.db.search(&query_embedding, limit * 2)?;

        let mut results: Vec<SearchResult> = raw_results
            .into_iter()
            .map(|(record, score)| {
                let boosted_score = compute_boosted_score(score, &record, boost);
                SearchResult {
                    id: record.id,
                    path: record.path,
                    title: record.title,
                    gist: record.gist,
                    note_type: record.note_type,
                    area: record.area,
                    score: boosted_score,
                }
            })
            .collect();

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);
        Ok(results)
    }

    /// Index all notes in vault
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub fn index_note(&mut self, note: &Note) -> Result<bool> {
        let gist = match note.gist() {
            Some(g) if !g.is_empty() => g,
            _ => return Ok(false),
        };

        let embedding = self.embedder.embed(gist)?;

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
    #[allow(dead_code)]
    pub fn get_stats(&self) -> Result<IndexStats> {
        self.db.get_stats()
    }
}

#[allow(dead_code)]
fn compute_boosted_score(semantic_score: f32, candidate: &NoteRecord, boost: &BoostOptions) -> f32 {
    const SEMANTIC_WEIGHT: f32 = 0.7;
    const METADATA_WEIGHT: f32 = 0.3;
    const TYPE_BOOST: f32 = 0.5;
    const AREA_BOOST: f32 = 0.5;

    let mut metadata_score = 0.0;

    if boost.boost_type {
        if let (Some(src), Some(cand)) = (&boost.source_type, &candidate.note_type) {
            if src == cand {
                metadata_score += TYPE_BOOST;
            }
        }
    }

    if boost.boost_area {
        if let (Some(src), Some(cand)) = (&boost.source_area, &candidate.area) {
            if src == cand {
                metadata_score += AREA_BOOST;
            }
        }
    }

    SEMANTIC_WEIGHT * semantic_score + METADATA_WEIGHT * metadata_score
}

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
