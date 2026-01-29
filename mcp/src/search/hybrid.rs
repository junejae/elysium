//! Hybrid Search Engine - combines semantic (HNSW) and keyword (BM25) search
//!
//! Supports three search modes:
//! - Hybrid: RRF fusion of BM25 + Semantic results (default)
//! - Semantic: HNSW vector search only (existing behavior)
//! - Keyword: BM25 text search only

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::bm25::Bm25Index;
use super::engine::SearchResult;
use super::plugin_index::{NoteRecord, PluginSearchEngine};

// ============================================================================
// Search Mode
// ============================================================================

/// Search mode selection for hybrid search engine
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum SearchMode {
    /// BM25 + Semantic with RRF fusion (default)
    #[default]
    Hybrid,
    /// HNSW semantic search only (existing behavior)
    Semantic,
    /// BM25 keyword search only
    Keyword,
}

impl SearchMode {
    /// Parse search mode from string
    ///
    /// # Examples
    /// ```
    /// use elysium_mcp::search::SearchMode;
    /// assert_eq!(SearchMode::from_str("semantic"), SearchMode::Semantic);
    /// assert_eq!(SearchMode::from_str("keyword"), SearchMode::Keyword);
    /// assert_eq!(SearchMode::from_str("bm25"), SearchMode::Keyword);
    /// assert_eq!(SearchMode::from_str("hybrid"), SearchMode::Hybrid);
    /// assert_eq!(SearchMode::from_str("unknown"), SearchMode::Hybrid); // default
    /// ```
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "semantic" => SearchMode::Semantic,
            "keyword" | "bm25" => SearchMode::Keyword,
            _ => SearchMode::Hybrid, // default
        }
    }
}

// ============================================================================
// Hybrid Configuration
// ============================================================================

/// Configuration for hybrid search fusion
#[derive(Debug, Clone)]
pub struct HybridConfig {
    /// Weight for BM25 results in RRF fusion (default: 0.3)
    pub bm25_weight: f32,
    /// Weight for semantic results in RRF fusion (default: 0.7)
    pub semantic_weight: f32,
    /// RRF k parameter - controls rank contribution decay (default: 60)
    pub rrf_k: usize,
}

impl Default for HybridConfig {
    fn default() -> Self {
        Self {
            bm25_weight: 0.3,
            semantic_weight: 0.7,
            rrf_k: 60,
        }
    }
}

impl HybridConfig {
    /// Create new config with custom weights
    pub fn with_weights(bm25_weight: f32, semantic_weight: f32) -> Self {
        Self {
            bm25_weight,
            semantic_weight,
            ..Default::default()
        }
    }
}

// ============================================================================
// RRF Fusion
// ============================================================================

/// RRF (Reciprocal Rank Fusion) algorithm
///
/// Combines ranked results from multiple sources using the formula:
/// score(doc) = sum(weight / (k + rank))
///
/// This is a rank-based fusion that doesn't require score normalization,
/// making it ideal for combining results from different scoring systems.
///
/// # Arguments
/// * `semantic_results` - Results from semantic search as (path, score) tuples
/// * `bm25_results` - Results from BM25 search as (path, score) tuples
/// * `config` - Hybrid search configuration with weights and k parameter
///
/// # Returns
/// Fused results sorted by combined RRF score in descending order
pub fn fuse_rrf(
    semantic_results: Vec<(String, f32)>,
    bm25_results: Vec<(String, f32)>,
    config: &HybridConfig,
) -> Vec<(String, f32)> {
    let mut scores: HashMap<String, f32> = HashMap::new();
    let k = config.rrf_k as f32;

    // Add semantic search contributions
    for (rank, (path, _score)) in semantic_results.into_iter().enumerate() {
        let rrf_score = config.semantic_weight / (k + (rank + 1) as f32);
        *scores.entry(path).or_insert(0.0) += rrf_score;
    }

    // Add BM25 search contributions
    for (rank, (path, _score)) in bm25_results.into_iter().enumerate() {
        let rrf_score = config.bm25_weight / (k + (rank + 1) as f32);
        *scores.entry(path).or_insert(0.0) += rrf_score;
    }

    // Sort by fused score descending
    let mut results: Vec<(String, f32)> = scores.into_iter().collect();
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    results
}

// ============================================================================
// Hybrid Search Engine
// ============================================================================

/// Hybrid search engine combining semantic and keyword search
///
/// Uses the Obsidian plugin's HNSW index for semantic search and
/// optionally builds a BM25 index for keyword search.
pub struct HybridSearchEngine {
    /// Semantic search engine (HNSW-based)
    semantic: PluginSearchEngine,
    /// BM25 keyword search index (lazy loaded)
    bm25: Option<Bm25Index>,
    /// Hybrid search configuration
    config: HybridConfig,
    /// Vault path for BM25 index building
    vault_path: PathBuf,
}

impl HybridSearchEngine {
    /// Create new hybrid search engine
    ///
    /// Loads the semantic search engine from the Obsidian plugin index.
    /// BM25 index is built lazily on first keyword/hybrid search.
    ///
    /// # Arguments
    /// * `vault_path` - Path to the Obsidian vault
    ///
    /// # Errors
    /// Returns error if plugin index cannot be loaded
    pub fn new(vault_path: &Path) -> Result<Self> {
        let semantic =
            PluginSearchEngine::load(vault_path).context("Failed to load plugin search engine")?;

        Ok(Self {
            semantic,
            bm25: None,
            config: HybridConfig::default(),
            vault_path: vault_path.to_path_buf(),
        })
    }

    /// Create with custom configuration
    #[allow(dead_code)]
    pub fn with_config(vault_path: &Path, config: HybridConfig) -> Result<Self> {
        let mut engine = Self::new(vault_path)?;
        engine.config = config;
        Ok(engine)
    }

    /// Get current configuration
    #[allow(dead_code)]
    pub fn config(&self) -> &HybridConfig {
        &self.config
    }

    /// Update configuration
    #[allow(dead_code)]
    pub fn set_config(&mut self, config: HybridConfig) {
        self.config = config;
    }

    /// Search with specified mode
    ///
    /// # Arguments
    /// * `query` - Search query string
    /// * `limit` - Maximum number of results
    /// * `mode` - Search mode (Hybrid, Semantic, or Keyword)
    ///
    /// # Returns
    /// Vector of search results sorted by relevance
    pub fn search(
        &mut self,
        query: &str,
        limit: usize,
        mode: SearchMode,
    ) -> Result<Vec<SearchResult>> {
        match mode {
            SearchMode::Semantic => self.search_semantic(query, limit),
            SearchMode::Keyword => self.search_keyword(query, limit),
            SearchMode::Hybrid => self.search_hybrid(query, limit),
        }
    }

    /// Semantic search only (HNSW)
    fn search_semantic(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        self.semantic.search(query, limit)
    }

    /// Keyword search only (BM25)
    fn search_keyword(&mut self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        self.ensure_bm25_index()?;

        let bm25 = self.bm25.as_ref().unwrap();
        let bm25_results = bm25.search(query, limit)?;

        // Convert BM25 results to SearchResult
        self.convert_bm25_results(bm25_results)
    }

    /// Hybrid search (RRF fusion of semantic + BM25)
    fn search_hybrid(&mut self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        self.ensure_bm25_index()?;

        // Get more results from each source for better fusion
        let fetch_limit = limit * 3;

        // Get semantic results
        let semantic_results = self.semantic.search(query, fetch_limit)?;
        let semantic_pairs: Vec<(String, f32)> = semantic_results
            .iter()
            .map(|r| (r.path.clone(), r.score))
            .collect();

        // Get BM25 results
        let bm25 = self.bm25.as_ref().unwrap();
        let bm25_pairs = bm25.search(query, fetch_limit)?;

        // Fuse results with RRF
        let fused = fuse_rrf(semantic_pairs, bm25_pairs, &self.config);

        // Convert fused results to SearchResult, limited to requested count
        self.convert_fused_results(fused, limit)
    }

    /// Ensure BM25 index is built (lazy loading)
    fn ensure_bm25_index(&mut self) -> Result<()> {
        if self.bm25.is_none() {
            // Collect notes from semantic engine for BM25 indexing
            let notes: Vec<NoteRecord> = self.semantic.iter_notes().cloned().collect();

            // Build BM25 index in a subdirectory of the vault
            let bm25_index_dir = self.vault_path.join(".obsidian/plugins/elysium/bm25_index");

            let bm25 = Bm25Index::build_from_notes(&notes, &bm25_index_dir)
                .context("Failed to build BM25 index")?;
            self.bm25 = Some(bm25);
        }
        Ok(())
    }

    /// Convert BM25 results to SearchResult
    fn convert_bm25_results(&self, results: Vec<(String, f32)>) -> Result<Vec<SearchResult>> {
        let mut search_results = Vec::with_capacity(results.len());

        for (path, score) in results {
            if let Some(note) = self.semantic.get_note(&path) {
                search_results.push(SearchResult {
                    id: path.clone(),
                    path,
                    title: note
                        .path
                        .rsplit('/')
                        .next()
                        .unwrap_or(&note.path)
                        .trim_end_matches(".md")
                        .to_string(),
                    gist: Some(note.gist.clone()),
                    note_type: note.fields.get("type").cloned(),
                    area: note.fields.get("area").cloned(),
                    score,
                });
            }
        }

        Ok(search_results)
    }

    /// Convert fused RRF results to SearchResult
    fn convert_fused_results(
        &self,
        results: Vec<(String, f32)>,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let mut search_results = Vec::with_capacity(limit.min(results.len()));

        for (path, score) in results.into_iter().take(limit) {
            if let Some(note) = self.semantic.get_note(&path) {
                search_results.push(SearchResult {
                    id: path.clone(),
                    path,
                    title: note
                        .path
                        .rsplit('/')
                        .next()
                        .unwrap_or(&note.path)
                        .trim_end_matches(".md")
                        .to_string(),
                    gist: Some(note.gist.clone()),
                    note_type: note.fields.get("type").cloned(),
                    area: note.fields.get("area").cloned(),
                    score,
                });
            }
        }

        Ok(search_results)
    }

    /// Get semantic engine reference
    #[allow(dead_code)]
    pub fn semantic_engine(&self) -> &PluginSearchEngine {
        &self.semantic
    }

    /// Check if BM25 index is loaded
    #[allow(dead_code)]
    pub fn has_bm25_index(&self) -> bool {
        self.bm25.is_some()
    }

    /// Get note count
    #[allow(dead_code)]
    pub fn note_count(&self) -> usize {
        self.semantic.note_count()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_mode_from_str() {
        assert_eq!(SearchMode::from_str("semantic"), SearchMode::Semantic);
        assert_eq!(SearchMode::from_str("Semantic"), SearchMode::Semantic);
        assert_eq!(SearchMode::from_str("SEMANTIC"), SearchMode::Semantic);

        assert_eq!(SearchMode::from_str("keyword"), SearchMode::Keyword);
        assert_eq!(SearchMode::from_str("bm25"), SearchMode::Keyword);
        assert_eq!(SearchMode::from_str("BM25"), SearchMode::Keyword);

        assert_eq!(SearchMode::from_str("hybrid"), SearchMode::Hybrid);
        assert_eq!(SearchMode::from_str(""), SearchMode::Hybrid);
        assert_eq!(SearchMode::from_str("unknown"), SearchMode::Hybrid);
    }

    #[test]
    fn test_search_mode_default() {
        assert_eq!(SearchMode::default(), SearchMode::Hybrid);
    }

    #[test]
    fn test_hybrid_config_default() {
        let config = HybridConfig::default();
        assert_eq!(config.bm25_weight, 0.3);
        assert_eq!(config.semantic_weight, 0.7);
        assert_eq!(config.rrf_k, 60);
    }

    #[test]
    fn test_hybrid_config_with_weights() {
        let config = HybridConfig::with_weights(0.5, 0.5);
        assert_eq!(config.bm25_weight, 0.5);
        assert_eq!(config.semantic_weight, 0.5);
        assert_eq!(config.rrf_k, 60); // default k
    }

    #[test]
    fn test_fuse_rrf_empty() {
        let config = HybridConfig::default();
        let result = fuse_rrf(vec![], vec![], &config);
        assert!(result.is_empty());
    }

    #[test]
    fn test_fuse_rrf_semantic_only() {
        let config = HybridConfig::default();
        let semantic = vec![("doc1".to_string(), 0.9), ("doc2".to_string(), 0.8)];
        let result = fuse_rrf(semantic, vec![], &config);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, "doc1");
        assert_eq!(result[1].0, "doc2");
    }

    #[test]
    fn test_fuse_rrf_bm25_only() {
        let config = HybridConfig::default();
        let bm25 = vec![("doc1".to_string(), 5.0), ("doc2".to_string(), 3.0)];
        let result = fuse_rrf(vec![], bm25, &config);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, "doc1");
        assert_eq!(result[1].0, "doc2");
    }

    #[test]
    fn test_fuse_rrf_combined() {
        let config = HybridConfig::default();

        // Semantic: doc1 rank 1, doc2 rank 2
        let semantic = vec![("doc1".to_string(), 0.9), ("doc2".to_string(), 0.8)];

        // BM25: doc2 rank 1, doc3 rank 2
        let bm25 = vec![("doc2".to_string(), 5.0), ("doc3".to_string(), 3.0)];

        let result = fuse_rrf(semantic, bm25, &config);

        // doc2 should be first (appears in both lists)
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].0, "doc2"); // highest combined score
    }

    #[test]
    fn test_fuse_rrf_score_calculation() {
        let config = HybridConfig {
            bm25_weight: 0.5,
            semantic_weight: 0.5,
            rrf_k: 60,
        };

        // Single doc in both lists at rank 1
        let semantic = vec![("doc1".to_string(), 0.9)];
        let bm25 = vec![("doc1".to_string(), 5.0)];

        let result = fuse_rrf(semantic, bm25, &config);

        // Expected score: 0.5/(60+1) + 0.5/(60+1) = 1.0/61
        let expected_score = 1.0 / 61.0;
        assert!((result[0].1 - expected_score).abs() < 0.0001);
    }
}
