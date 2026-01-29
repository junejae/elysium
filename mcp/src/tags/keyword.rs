//! Keyword extraction using Model2Vec token embeddings
//!
//! Extracts representative keywords from text by comparing
//! individual token embeddings against the document embedding.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

use safetensors::SafeTensors;
use tokenizers::Tokenizer;

/// Keyword extractor using Model2Vec embeddings
pub struct KeywordExtractor {
    tokenizer: Tokenizer,
    embeddings: Vec<Vec<f32>>,
    embedding_dim: usize,
    #[allow(dead_code)]
    vocab: HashMap<String, u32>,
}

impl KeywordExtractor {
    /// Load from Model2Vec model directory
    pub fn from_model_path(model_path: &Path) -> Result<Self> {
        // Load tokenizer
        let tok_path = model_path.join("tokenizer.json");
        let tokenizer = Tokenizer::from_file(&tok_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {}", e))?;

        // Build vocab lookup
        let vocab = tokenizer.get_vocab(false);

        // Load embeddings from safetensors
        let mdl_path = model_path.join("model.safetensors");
        let model_bytes = std::fs::read(&mdl_path).context("Failed to read model.safetensors")?;
        let safet =
            SafeTensors::deserialize(&model_bytes).context("Failed to parse safetensors")?;

        let tensor = safet
            .tensor("embeddings")
            .or_else(|_| safet.tensor("0"))
            .context("No embeddings tensor found")?;

        let shape = tensor.shape();
        let cols = shape[1];

        // Convert to f32 vectors
        let raw = tensor.data();
        let floats: Vec<f32> = raw
            .chunks_exact(4)
            .map(|bs: &[u8]| f32::from_le_bytes(bs.try_into().unwrap()))
            .collect();

        let embeddings: Vec<Vec<f32>> = floats
            .chunks_exact(cols)
            .map(|chunk: &[f32]| chunk.to_vec())
            .collect();

        Ok(Self {
            tokenizer,
            embeddings,
            embedding_dim: cols,
            vocab,
        })
    }

    /// Load from default HuggingFace cache path
    pub fn from_default_cache() -> Result<Self> {
        let home = std::env::var("HOME").context("HOME not set")?;
        let cache_path = Path::new(&home)
            .join(".cache/huggingface/hub/models--minishlab--potion-multilingual-128M/snapshots");

        // Find snapshot directory
        let snapshot_dir = std::fs::read_dir(&cache_path)?
            .filter_map(|e| e.ok())
            .find(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .context("No snapshot found")?;

        Self::from_model_path(&snapshot_dir.path())
    }

    /// Get embedding for a token ID
    fn get_embedding(&self, token_id: u32) -> Option<&[f32]> {
        self.embeddings.get(token_id as usize).map(|v| v.as_slice())
    }

    /// Compute document embedding (mean of token embeddings)
    fn compute_doc_embedding(&self, token_ids: &[u32]) -> Vec<f32> {
        let mut doc_emb = vec![0.0f32; self.embedding_dim];
        let mut count = 0;

        for &id in token_ids {
            if let Some(emb) = self.get_embedding(id) {
                for (i, &v) in emb.iter().enumerate() {
                    doc_emb[i] += v;
                }
                count += 1;
            }
        }

        if count > 0 {
            for v in &mut doc_emb {
                *v /= count as f32;
            }
        }

        doc_emb
    }

    /// Cosine similarity
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a > 0.0 && norm_b > 0.0 {
            dot / (norm_a * norm_b)
        } else {
            0.0
        }
    }

    /// Extract keywords from text
    ///
    /// Returns keywords sorted by relevance (similarity to document embedding)
    pub fn extract_keywords(&self, text: &str, limit: usize) -> Result<Vec<Keyword>> {
        // Tokenize
        let encoding = self
            .tokenizer
            .encode(text, false)
            .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;

        let token_ids: Vec<u32> = encoding.get_ids().to_vec();
        let tokens: Vec<String> = encoding
            .get_tokens()
            .iter()
            .map(|s: &String| s.clone())
            .collect();

        if token_ids.is_empty() {
            return Ok(vec![]);
        }

        // Compute document embedding
        let doc_emb = self.compute_doc_embedding(&token_ids);

        // Merge subwords into complete words
        let words = self.merge_subwords(&tokens, &token_ids);

        // Score each unique word
        let mut word_scores: HashMap<String, f32> = HashMap::new();

        for word in &words {
            // Skip special tokens and short words
            if word.text.starts_with('[') || word.text.starts_with('<') || word.text.len() < 2 {
                continue;
            }

            // Skip if already scored
            let clean_word = clean_token(&word.text);
            if clean_word.is_empty() || word_scores.contains_key(&clean_word) {
                continue;
            }

            // Compute word embedding as mean of subword embeddings
            let word_emb = self.compute_word_embedding(&word.token_ids);
            if word_emb.iter().all(|&v| v == 0.0) {
                continue;
            }

            let similarity = Self::cosine_similarity(&word_emb, &doc_emb);
            word_scores.insert(clean_word, similarity);
        }

        // Sort by score
        let mut keywords: Vec<Keyword> = word_scores
            .into_iter()
            .map(|(token, score)| Keyword { token, score })
            .filter(|k| k.score > 0.1)
            .collect();

        keywords.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        keywords.truncate(limit);

        Ok(keywords)
    }

    /// Merge subword tokens into complete words
    fn merge_subwords(&self, tokens: &[String], token_ids: &[u32]) -> Vec<Word> {
        let mut words = Vec::new();
        let mut current_word = String::new();
        let mut current_ids = Vec::new();

        for i in 0..tokens.len() {
            let token = &tokens[i];
            let token_id = token_ids[i];

            // Check if this is a word start (has prefix) or continuation
            let is_word_start = token.starts_with('▁')  // SentencePiece
                || token.starts_with('Ġ')  // GPT-style
                || (i == 0 && !token.starts_with("##")); // First token or BERT continuation

            if is_word_start {
                // Save previous word if any
                if !current_word.is_empty() {
                    words.push(Word {
                        text: current_word.clone(),
                        token_ids: current_ids.clone(),
                    });
                }
                // Start new word
                current_word = clean_token(token);
                current_ids = vec![token_id];
            } else {
                // Continuation token - merge with current word
                let cleaned = token.trim_start_matches("##"); // BERT continuation
                current_word.push_str(cleaned);
                current_ids.push(token_id);
            }
        }

        // Don't forget the last word
        if !current_word.is_empty() {
            words.push(Word {
                text: current_word,
                token_ids: current_ids,
            });
        }

        words
    }

    /// Compute embedding for a word (mean of subword embeddings)
    fn compute_word_embedding(&self, token_ids: &[u32]) -> Vec<f32> {
        let mut word_emb = vec![0.0f32; self.embedding_dim];
        let mut count = 0;

        for &id in token_ids {
            if let Some(emb) = self.get_embedding(id) {
                for (i, &v) in emb.iter().enumerate() {
                    word_emb[i] += v;
                }
                count += 1;
            }
        }

        if count > 0 {
            for v in &mut word_emb {
                *v /= count as f32;
            }
        }

        word_emb
    }
}

/// Reconstructed word from subword tokens
struct Word {
    text: String,
    token_ids: Vec<u32>,
}

/// Clean token (remove BPE markers like Ġ, ##, etc.)
fn clean_token(token: &str) -> String {
    token
        .trim_start_matches('Ġ') // GPT-style
        .trim_start_matches("##") // BERT-style
        .trim_start_matches('▁') // SentencePiece
        .to_lowercase()
}

/// Extracted keyword with relevance score
#[derive(Debug, Clone)]
pub struct Keyword {
    pub token: String,
    pub score: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires model
    fn test_keyword_extraction() {
        let extractor = KeywordExtractor::from_default_cache().unwrap();

        let keywords = extractor
            .extract_keywords("GPU memory optimization for CUDA programming", 5)
            .unwrap();

        println!("Keywords: {:?}", keywords);

        let tokens: Vec<_> = keywords.iter().map(|k| k.token.as_str()).collect();
        assert!(tokens.contains(&"gpu") || tokens.contains(&"cuda"));
    }
}
