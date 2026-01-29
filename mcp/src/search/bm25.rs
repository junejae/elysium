//! BM25 Full-Text Search using Tantivy
//!
//! Provides keyword-based search with TF-IDF scoring and field boosting.
//! Complements semantic search for hybrid search functionality.

use anyhow::{Context, Result};
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Schema, Value, STORED, STRING, TEXT};
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy};

use super::plugin_index::{NoteRecord, PluginIndexReader};

// ============================================================================
// Configuration
// ============================================================================

/// BM25 search configuration with field boost weights
#[derive(Debug, Clone)]
pub struct Bm25Config {
    /// Boost weight for title field (default: 3.0)
    pub title_boost: f32,
    /// Boost weight for gist field (default: 2.0)
    pub gist_boost: f32,
    /// Boost weight for tags field (default: 1.5)
    pub tags_boost: f32,
}

impl Default for Bm25Config {
    fn default() -> Self {
        Self {
            title_boost: 3.0,
            gist_boost: 2.0,
            tags_boost: 1.5,
        }
    }
}

// ============================================================================
// BM25 Index
// ============================================================================

/// BM25 full-text search index using Tantivy
pub struct Bm25Index {
    index: Index,
    reader: IndexReader,
    // Schema fields
    title_field: tantivy::schema::Field,
    gist_field: tantivy::schema::Field,
    tags_field: tantivy::schema::Field,
    path_field: tantivy::schema::Field,
    // Configuration
    config: Bm25Config,
}

impl Bm25Index {
    /// Build index from note records
    ///
    /// Creates a new index at the specified directory from the given notes.
    pub fn build_from_notes(notes: &[NoteRecord], index_dir: &Path) -> Result<Self> {
        Self::build_from_notes_with_config(notes, index_dir, Bm25Config::default())
    }

    /// Build index from note records with custom configuration
    pub fn build_from_notes_with_config(
        notes: &[NoteRecord],
        index_dir: &Path,
        config: Bm25Config,
    ) -> Result<Self> {
        // Create index directory
        std::fs::create_dir_all(index_dir).with_context(|| {
            format!("Failed to create index directory: {}", index_dir.display())
        })?;

        // Build schema
        let (schema, title_field, gist_field, tags_field, path_field) = Self::build_schema();

        // Create or open index
        let index = Index::create_in_dir(index_dir, schema.clone())
            .or_else(|_| {
                // If index exists, open and clear it
                let index = Index::open_in_dir(index_dir)?;
                Ok::<Index, tantivy::TantivyError>(index)
            })
            .with_context(|| format!("Failed to create index at {}", index_dir.display()))?;

        // Index all notes
        let mut writer: IndexWriter = index
            .writer(50_000_000) // 50MB heap
            .context("Failed to create index writer")?;

        // Clear existing documents
        writer.delete_all_documents()?;

        for note in notes {
            let title = Self::extract_title(&note.path);
            let tags_text = note.tags.as_ref().map(|t| t.join(" ")).unwrap_or_default();

            writer.add_document(doc!(
                title_field => title,
                gist_field => note.gist.as_str(),
                tags_field => tags_text,
                path_field => note.path.as_str(),
            ))?;
        }

        writer.commit().context("Failed to commit index")?;

        // Create reader
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .context("Failed to create index reader")?;

        Ok(Self {
            index,
            reader,
            title_field,
            gist_field,
            tags_field,
            path_field,
            config,
        })
    }

    /// Load existing index from directory
    pub fn load(index_dir: &Path) -> Result<Self> {
        Self::load_with_config(index_dir, Bm25Config::default())
    }

    /// Load existing index with custom configuration
    pub fn load_with_config(index_dir: &Path, config: Bm25Config) -> Result<Self> {
        let index = Index::open_in_dir(index_dir)
            .with_context(|| format!("Failed to open index at {}", index_dir.display()))?;

        let schema = index.schema();

        let title_field = schema
            .get_field("title")
            .context("Schema missing 'title' field")?;
        let gist_field = schema
            .get_field("gist")
            .context("Schema missing 'gist' field")?;
        let tags_field = schema
            .get_field("tags")
            .context("Schema missing 'tags' field")?;
        let path_field = schema
            .get_field("path")
            .context("Schema missing 'path' field")?;

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .context("Failed to create index reader")?;

        Ok(Self {
            index,
            reader,
            title_field,
            gist_field,
            tags_field,
            path_field,
            config,
        })
    }

    /// Build BM25 index from Obsidian vault
    ///
    /// Reads notes from the plugin index and builds a BM25 index
    /// in the plugin's index directory.
    ///
    /// # Arguments
    /// * `vault_path` - Path to the Obsidian vault
    ///
    /// # Returns
    /// Built BM25 index ready for searching
    #[allow(dead_code)]
    pub fn build(vault_path: &Path) -> Result<Self> {
        Self::build_with_config(vault_path, Bm25Config::default())
    }

    /// Build BM25 index from Obsidian vault with custom configuration
    #[allow(dead_code)]
    pub fn build_with_config(vault_path: &Path, config: Bm25Config) -> Result<Self> {
        // Read notes from plugin index
        let reader = PluginIndexReader::new(vault_path);
        let notes = reader
            .load_notes()
            .context("Failed to load notes from plugin index")?;

        // Build index in plugin's index directory
        let index_dir = vault_path.join(".obsidian/plugins/elysium/index/bm25");

        Self::build_from_notes_with_config(&notes, &index_dir, config)
    }

    /// Search the index with query string
    ///
    /// Returns vector of (path, score) tuples sorted by relevance.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<(String, f32)>> {
        let searcher = self.reader.searcher();

        // Build query parser with field boosts
        let mut query_parser = QueryParser::for_index(
            &self.index,
            vec![self.title_field, self.gist_field, self.tags_field],
        );

        // Set field boosts
        query_parser.set_field_boost(self.title_field, self.config.title_boost);
        query_parser.set_field_boost(self.gist_field, self.config.gist_boost);
        query_parser.set_field_boost(self.tags_field, self.config.tags_boost);

        // Parse query (lenient mode to handle special characters)
        let parsed_query = query_parser
            .parse_query(query)
            .with_context(|| format!("Failed to parse query: {}", query))?;

        // Execute search
        let top_docs = searcher
            .search(&parsed_query, &TopDocs::with_limit(limit))
            .context("Search execution failed")?;

        // Extract results
        let mut results = Vec::with_capacity(top_docs.len());

        for (score, doc_address) in top_docs {
            let retrieved_doc: tantivy::TantivyDocument = searcher
                .doc(doc_address)
                .context("Failed to retrieve document")?;

            if let Some(path_value) = retrieved_doc.get_first(self.path_field) {
                if let Some(path_str) = path_value.as_str() {
                    results.push((path_str.to_string(), score));
                }
            }
        }

        Ok(results)
    }

    /// Get the number of documents in the index
    pub fn num_docs(&self) -> u64 {
        self.reader.searcher().num_docs()
    }

    /// Get current configuration
    pub fn config(&self) -> &Bm25Config {
        &self.config
    }

    // ------------------------------------------------------------------------
    // Private helpers
    // ------------------------------------------------------------------------

    /// Build the tantivy schema
    fn build_schema() -> (
        Schema,
        tantivy::schema::Field,
        tantivy::schema::Field,
        tantivy::schema::Field,
        tantivy::schema::Field,
    ) {
        let mut schema_builder = Schema::builder();

        // TEXT fields: tokenized and indexed for full-text search, not stored
        let title_field = schema_builder.add_text_field("title", TEXT);
        let gist_field = schema_builder.add_text_field("gist", TEXT);
        let tags_field = schema_builder.add_text_field("tags", TEXT);

        // STRING | STORED: stored for retrieval, indexed as single token
        let path_field = schema_builder.add_text_field("path", STRING | STORED);

        let schema = schema_builder.build();

        (schema, title_field, gist_field, tags_field, path_field)
    }

    /// Extract title from file path
    fn extract_title(path: &str) -> String {
        path.rsplit('/')
            .next()
            .unwrap_or(path)
            .trim_end_matches(".md")
            .to_string()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn create_test_notes() -> Vec<NoteRecord> {
        vec![
            NoteRecord {
                path: "Notes/Rust Programming.md".to_string(),
                gist: "Rust is a systems programming language focused on safety and performance"
                    .to_string(),
                mtime: 1704067200,
                indexed: true,
                fields: HashMap::new(),
                tags: Some(vec!["rust".to_string(), "programming".to_string()]),
            },
            NoteRecord {
                path: "Notes/Python Basics.md".to_string(),
                gist: "Python is a high-level programming language known for its simplicity"
                    .to_string(),
                mtime: 1704067300,
                indexed: true,
                fields: HashMap::new(),
                tags: Some(vec!["python".to_string(), "programming".to_string()]),
            },
            NoteRecord {
                path: "Notes/Machine Learning.md".to_string(),
                gist: "Machine learning is a subset of AI that enables systems to learn from data"
                    .to_string(),
                mtime: 1704067400,
                indexed: true,
                fields: HashMap::new(),
                tags: Some(vec!["ml".to_string(), "ai".to_string()]),
            },
        ]
    }

    #[test]
    fn test_build_and_search() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let notes = create_test_notes();

        let index = Bm25Index::build_from_notes(&notes, temp_dir.path())?;

        assert_eq!(index.num_docs(), 3);

        // Search for "rust"
        let results = index.search("rust", 10)?;
        assert!(!results.is_empty());
        assert_eq!(results[0].0, "Notes/Rust Programming.md");

        // Search for "programming" - should match multiple
        let results = index.search("programming", 10)?;
        assert!(results.len() >= 2);

        Ok(())
    }

    #[test]
    fn test_load_existing_index() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let notes = create_test_notes();

        // Build index
        let _index = Bm25Index::build_from_notes(&notes, temp_dir.path())?;

        // Load index
        let loaded_index = Bm25Index::load(temp_dir.path())?;
        assert_eq!(loaded_index.num_docs(), 3);

        // Search should still work
        let results = loaded_index.search("python", 5)?;
        assert!(!results.is_empty());

        Ok(())
    }

    #[test]
    fn test_custom_config() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let notes = create_test_notes();

        let config = Bm25Config {
            title_boost: 5.0,
            gist_boost: 1.0,
            tags_boost: 2.0,
        };

        let index = Bm25Index::build_from_notes_with_config(&notes, temp_dir.path(), config)?;

        assert_eq!(index.config().title_boost, 5.0);
        assert_eq!(index.config().gist_boost, 1.0);
        assert_eq!(index.config().tags_boost, 2.0);

        Ok(())
    }

    #[test]
    fn test_extract_title() {
        assert_eq!(Bm25Index::extract_title("Notes/Test Note.md"), "Test Note");
        assert_eq!(
            Bm25Index::extract_title("Notes/Subfolder/Deep Note.md"),
            "Deep Note"
        );
        assert_eq!(Bm25Index::extract_title("Simple.md"), "Simple");
    }

    #[test]
    fn test_empty_index() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let notes: Vec<NoteRecord> = vec![];

        let index = Bm25Index::build_from_notes(&notes, temp_dir.path())?;
        assert_eq!(index.num_docs(), 0);

        let results = index.search("anything", 10)?;
        assert!(results.is_empty());

        Ok(())
    }

    #[test]
    fn test_tags_search() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let notes = create_test_notes();

        let index = Bm25Index::build_from_notes(&notes, temp_dir.path())?;

        // Search by tag
        let results = index.search("ml", 10)?;
        assert!(!results.is_empty());
        assert_eq!(results[0].0, "Notes/Machine Learning.md");

        Ok(())
    }
}
