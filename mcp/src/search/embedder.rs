//! Embedder trait and implementations for semantic search
//!
//! Provides abstraction over different embedding models:
//! - HtpEmbedder: Harmonic Token Projection (built-in, no model file)
//! - Model2VecEmbedder: Neural network based (requires model download)

use anyhow::{Context, Result};
use model2vec::Model2Vec;
use std::path::Path;

/// Embedding model abstraction
pub trait Embedder: Send + Sync {
    /// Generate embedding for a single text
    fn embed(&self, text: &str) -> Result<Vec<f32>>;

    /// Generate embeddings for multiple texts
    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;

    /// Get embedding dimension
    fn dimension(&self) -> usize;

    /// Get model name/identifier
    fn name(&self) -> &str;
}

// ============================================================================
// HTP Embedder (from embedding.rs)
// ============================================================================

use super::embedding::EmbeddingModel;

/// HTP (Harmonic Token Projection) Embedder wrapper
pub struct HtpEmbedder {
    model: EmbeddingModel,
}

impl HtpEmbedder {
    pub fn new() -> Self {
        Self {
            model: EmbeddingModel::new(),
        }
    }
}

impl Default for HtpEmbedder {
    fn default() -> Self {
        Self::new()
    }
}

impl Embedder for HtpEmbedder {
    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        self.model.embed(text)
    }

    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        self.model.embed_batch(texts)
    }

    fn dimension(&self) -> usize {
        super::embedding::EMBEDDING_DIM
    }

    fn name(&self) -> &str {
        "htp-384"
    }
}

// ============================================================================
// Model2Vec Embedder
// ============================================================================

/// Model2Vec embedding dimension (potion-base-8M uses 256d)
pub const MODEL2VEC_DIM: usize = 256;

/// Model2Vec based embedder for advanced semantic search
pub struct Model2VecEmbedder {
    model: Model2Vec,
    model_path: String,
}

impl Model2VecEmbedder {
    /// Load model from local path
    pub fn from_path(path: &Path) -> Result<Self> {
        let model = Model2Vec::from_pretrained(path.to_string_lossy().as_ref(), None, None)
            .with_context(|| format!("Failed to load Model2Vec from: {}", path.display()))?;

        Ok(Self {
            model,
            model_path: path.to_string_lossy().to_string(),
        })
    }

    /// Load model from HuggingFace Hub
    pub fn from_pretrained(model_id: &str) -> Result<Self> {
        let model = Model2Vec::from_pretrained(model_id, None, None)
            .with_context(|| format!("Failed to load Model2Vec: {}", model_id))?;

        Ok(Self {
            model,
            model_path: model_id.to_string(),
        })
    }
}

impl Embedder for Model2VecEmbedder {
    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let texts = [text];
        let embeddings = self.model.encode(&texts).context("Failed to encode text")?;
        Ok(embeddings.row(0).to_vec())
    }

    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let embeddings = self.model.encode(texts).context("Failed to encode texts")?;
        Ok(embeddings.rows().into_iter().map(|r| r.to_vec()).collect())
    }

    fn dimension(&self) -> usize {
        MODEL2VEC_DIM
    }

    fn name(&self) -> &str {
        "model2vec-256"
    }
}

// ============================================================================
// Factory function
// ============================================================================

use crate::core::config::DEFAULT_MODEL2VEC_MODEL;

/// Search configuration for embedder selection
#[derive(Debug, Clone)]
pub struct SearchConfig {
    pub use_advanced: bool,
    pub model_path: Option<String>,
    pub model_id: Option<String>,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            use_advanced: false,
            model_path: None,
            model_id: None,
        }
    }
}

/// Create embedder based on configuration
///
/// Priority:
/// 1. If use_advanced is false -> HtpEmbedder (default)
/// 2. If model_path is set -> load from local path
/// 3. If model_id is set -> download from HuggingFace Hub
/// 4. Otherwise -> use default model ID from HuggingFace Hub
pub fn create_embedder(config: &SearchConfig) -> Result<Box<dyn Embedder>> {
    if !config.use_advanced {
        return Ok(Box::new(HtpEmbedder::new()));
    }

    // Priority 1: Local path
    if let Some(path) = &config.model_path {
        let embedder = Model2VecEmbedder::from_path(Path::new(path))?;
        return Ok(Box::new(embedder));
    }

    // Priority 2: Model ID from config or default
    let model_id = config
        .model_id
        .as_deref()
        .unwrap_or(DEFAULT_MODEL2VEC_MODEL);

    let embedder = Model2VecEmbedder::from_pretrained(model_id)?;
    Ok(Box::new(embedder))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_htp_embedder() {
        let embedder = HtpEmbedder::new();

        let emb = embedder.embed("hello world").unwrap();
        assert_eq!(emb.len(), embedder.dimension());
        assert_eq!(embedder.name(), "htp-384");
    }

    #[test]
    fn test_create_embedder_htp() {
        let config = SearchConfig::default();
        let embedder = create_embedder(&config).unwrap();

        assert_eq!(embedder.dimension(), 384);
        assert_eq!(embedder.name(), "htp-384");
    }
}
