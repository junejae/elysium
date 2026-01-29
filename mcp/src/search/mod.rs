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

#[allow(unused_imports)]
pub use bm25::Bm25Index;
#[allow(unused_imports)]
pub use embedder::{create_embedder, Embedder, HtpEmbedder, Model2VecEmbedder, SearchConfig};
#[allow(unused_imports)]
pub use embedding::EmbeddingModel;
#[allow(unused_imports)]
pub use engine::{SearchEngine, SearchResult};
#[allow(unused_imports)]
pub use hybrid::{HybridConfig, HybridSearchEngine, SearchMode};
#[allow(unused_imports)]
pub use plugin_index::{PluginIndexReader, PluginSearchEngine};
#[allow(unused_imports)]
pub use vectordb::VectorDB;
