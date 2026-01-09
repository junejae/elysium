//! Model2Vec embedder wrapper for tag matching
//!
//! Uses the potion-multilingual-128M model for semantic embeddings.

use anyhow::{Context, Result};
use model2vec::Model2Vec;
use std::path::Path;

/// Default model for multilingual support (HuggingFace ID)
pub const DEFAULT_MODEL_HF: &str = "minishlab/potion-multilingual-128M";

/// Local cache path for the model
fn get_cached_model_path() -> Option<std::path::PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let cache_path = std::path::PathBuf::from(home)
        .join(".cache/huggingface/hub/models--minishlab--potion-multilingual-128M/snapshots");

    // Find the latest snapshot
    if cache_path.exists() {
        if let Ok(entries) = std::fs::read_dir(&cache_path) {
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    return Some(entry.path());
                }
            }
        }
    }
    None
}

/// Embedding dimension for potion-multilingual-128M
pub const EMBEDDING_DIM: usize = 256;

/// Tag embedder using Model2Vec
pub struct TagEmbedder {
    model: Model2Vec,
}

impl TagEmbedder {
    /// Load model from HuggingFace Hub
    pub fn from_pretrained(model_id: &str) -> Result<Self> {
        let model = Model2Vec::from_pretrained(model_id, None, None)
            .with_context(|| format!("Failed to load model: {}", model_id))?;

        Ok(Self { model })
    }

    /// Load model from local path
    pub fn from_path(path: &Path) -> Result<Self> {
        let model = Model2Vec::from_pretrained(path.to_string_lossy().as_ref(), None, None)
            .with_context(|| format!("Failed to load model from: {}", path.display()))?;

        Ok(Self { model })
    }

    /// Load default multilingual model
    /// Tries local cache first, then HuggingFace Hub
    pub fn default_multilingual() -> Result<Self> {
        // Try local cache first
        if let Some(cache_path) = get_cached_model_path() {
            return Self::from_path(&cache_path);
        }

        // Fall back to HuggingFace Hub
        Self::from_pretrained(DEFAULT_MODEL_HF)
    }

    /// Generate embedding for a single text
    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let texts = [text];
        let embeddings = self.model.encode(&texts).context("Failed to encode text")?;

        // Get first row as Vec<f32>
        Ok(embeddings.row(0).to_vec())
    }

    /// Generate embeddings for multiple texts
    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let embeddings = self.model.encode(texts).context("Failed to encode texts")?;

        Ok(embeddings.rows().into_iter().map(|r| r.to_vec()).collect())
    }

    /// Calculate cosine similarity between two embeddings
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return 0.0;
        }

        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a > 0.0 && norm_b > 0.0 {
            dot / (norm_a * norm_b)
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires model download
    fn test_embedder_basic() {
        let embedder = TagEmbedder::default_multilingual().unwrap();

        let emb1 = embedder.embed("GPU memory optimization").unwrap();
        let emb2 = embedder.embed("CUDA programming").unwrap();
        let emb3 = embedder.embed("cooking recipes").unwrap();

        assert_eq!(emb1.len(), EMBEDDING_DIM);

        // Similar topics should have higher similarity
        let sim_similar = TagEmbedder::cosine_similarity(&emb1, &emb2);
        let sim_different = TagEmbedder::cosine_similarity(&emb1, &emb3);

        println!("GPU-CUDA similarity: {}", sim_similar);
        println!("GPU-cooking similarity: {}", sim_different);

        assert!(sim_similar > sim_different);
    }

    #[test]
    #[ignore] // Requires model download
    fn test_korean_support() {
        let embedder = TagEmbedder::default_multilingual().unwrap();

        let emb_ko = embedder.embed("GPU 메모리 최적화").unwrap();
        let emb_en = embedder.embed("GPU memory optimization").unwrap();

        let similarity = TagEmbedder::cosine_similarity(&emb_ko, &emb_en);
        println!("Korean-English similarity: {}", similarity);

        // Should have reasonable similarity
        assert!(similarity > 0.5);
    }
}
