//! Semantic Search Engine for Second Brain
//!
//! Phase 1: Vector search using gist embeddings
//! Phase 2: + BM25 hybrid search (future)
//! Phase 3: + Knowledge graph (future)

pub mod embedding;
pub mod embedder;
pub mod engine;
pub mod vectordb;

pub use embedding::EmbeddingModel;
pub use embedder::{create_embedder, Embedder, HtpEmbedder, Model2VecEmbedder, SearchConfig};
pub use engine::{SearchEngine, SearchResult};
pub use vectordb::VectorDB;
