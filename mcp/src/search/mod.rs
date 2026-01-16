//! Semantic Search Engine for Second Brain
//!
//! Phase 1: Vector search using gist embeddings
//! Phase 2: + BM25 hybrid search
//! Phase 3: + Knowledge graph (future)

pub mod bm25;
pub mod embedder;
pub mod embedding;
pub mod engine;
pub mod hybrid;
pub mod plugin_index;
pub mod vectordb;

pub use bm25::Bm25Index;
pub use embedder::{create_embedder, Embedder, HtpEmbedder, Model2VecEmbedder, SearchConfig};
pub use embedding::EmbeddingModel;
pub use engine::{SearchEngine, SearchResult};
pub use hybrid::{HybridConfig, HybridSearchEngine, SearchMode};
pub use plugin_index::{PluginIndexReader, PluginSearchEngine};
pub use vectordb::VectorDB;
