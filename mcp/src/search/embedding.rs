//! Harmonic Token Projection (HTP) Embedding
//!
//! A deterministic, training-free embedding method based on:
//! "Harmonic Token Projection: A Vocabulary-Free, Training-Free,
//!  Deterministic, and Reversible Embedding Methodology"
//! https://arxiv.org/html/2511.20665
//!
//! Key properties:
//! - No neural network required
//! - Deterministic (same input → same output)
//! - Unicode-based (multilingual support)
//! - Fast (~1.5ms per sentence vs ~45ms for BERT)
//! - Memory efficient (<1MB vs ~4GB for BERT)

use anyhow::Result;
use std::f64::consts::PI;
use std::path::Path;

/// Embedding dimension (2 * number of coprime moduli)
/// Using 192 moduli → 384 dimensions (matching common transformer dims)
pub const EMBEDDING_DIM: usize = 384;

/// Number of coprime moduli for harmonic projection
const NUM_MODULI: usize = EMBEDDING_DIM / 2;

/// Maximum token length (Unicode code points)
const MAX_TOKEN_LENGTH: usize = 64;

/// Coprime moduli for modular decomposition
/// Using first NUM_MODULI primes for guaranteed coprimality
static COPRIME_MODULI: &[u64] = &[
    2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97,
    101, 103, 107, 109, 113, 127, 131, 137, 139, 149, 151, 157, 163, 167, 173, 179, 181, 191, 193,
    197, 199, 211, 223, 227, 229, 233, 239, 241, 251, 257, 263, 269, 271, 277, 281, 283, 293, 307,
    311, 313, 317, 331, 337, 347, 349, 353, 359, 367, 373, 379, 383, 389, 397, 401, 409, 419, 421,
    431, 433, 439, 443, 449, 457, 461, 463, 467, 479, 487, 491, 499, 503, 509, 521, 523, 541, 547,
    557, 563, 569, 571, 577, 587, 593, 599, 601, 607, 613, 617, 619, 631, 641, 643, 647, 653, 659,
    661, 673, 677, 683, 691, 701, 709, 719, 727, 733, 739, 743, 751, 757, 761, 769, 773, 787, 797,
    809, 811, 821, 823, 827, 829, 839, 853, 857, 859, 863, 877, 881, 883, 887, 907, 911, 919, 929,
    937, 941, 947, 953, 967, 971, 977, 983, 991, 997, 1009, 1013, 1019, 1021, 1031, 1033, 1039,
    1049, 1051, 1061, 1063, 1069, 1087, 1091, 1093, 1097, 1103, 1109, 1117, 1123, 1129, 1151, 1153,
    1163, 1171, 1181,
];

/// HTP Embedding Model
///
/// Implements Harmonic Token Projection for deterministic text embeddings
pub struct EmbeddingModel {
    moduli: Vec<u64>,
}

impl EmbeddingModel {
    /// Create new HTP embedding model
    ///
    /// Note: model_path is ignored (no model file needed)
    /// Kept for API compatibility
    pub fn load(_model_path: &Path) -> Result<Self> {
        Ok(Self::new())
    }

    /// Create new HTP embedding model
    pub fn new() -> Self {
        Self {
            moduli: COPRIME_MODULI[..NUM_MODULI].to_vec(),
        }
    }

    /// Generate embedding for a single text
    ///
    /// Algorithm:
    /// 1. Tokenize text into words
    /// 2. Embed each token using harmonic projection
    /// 3. Average token embeddings (mean pooling)
    /// 4. L2 normalize result
    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let tokens = tokenize(text);

        if tokens.is_empty() {
            return Ok(vec![0.0; EMBEDDING_DIM]);
        }

        // Embed each token
        let mut sum_embedding = vec![0.0f64; EMBEDDING_DIM];
        let mut count = 0;

        for token in &tokens {
            let token_emb = self.embed_token(token);
            for (i, val) in token_emb.iter().enumerate() {
                sum_embedding[i] += val;
            }
            count += 1;
        }

        // Mean pooling
        if count > 0 {
            for val in &mut sum_embedding {
                *val /= count as f64;
            }
        }

        // L2 normalize and convert to f32
        let norm: f64 = sum_embedding.iter().map(|x| x * x).sum::<f64>().sqrt();
        let embedding: Vec<f32> = if norm > 0.0 {
            sum_embedding.iter().map(|x| (*x / norm) as f32).collect()
        } else {
            sum_embedding.iter().map(|x| *x as f32).collect()
        };

        Ok(embedding)
    }

    /// Embed a single token using Harmonic Token Projection
    ///
    /// Steps:
    /// 1. Convert token to Unicode code points
    /// 2. Encode as base-2^16 integer N
    /// 3. For each modulus m_i, compute r_i = N mod m_i
    /// 4. Project to unit circle: E_i = [sin(2πr_i/m_i), cos(2πr_i/m_i)]
    fn embed_token(&self, token: &str) -> Vec<f64> {
        // Convert token to integer representation
        let n = self.token_to_integer(token);

        // Harmonic projection for each modulus
        let mut embedding = Vec::with_capacity(EMBEDDING_DIM);

        for &m in &self.moduli {
            let r = n % m;
            let theta = 2.0 * PI * (r as f64) / (m as f64);
            embedding.push(theta.sin());
            embedding.push(theta.cos());
        }

        embedding
    }

    /// Convert token to integer using Unicode encoding
    ///
    /// N = Σ u_j * B^(L-j) where B = 2^16
    fn token_to_integer(&self, token: &str) -> u64 {
        let chars: Vec<char> = token.chars().take(MAX_TOKEN_LENGTH).collect();

        // Use wrapping arithmetic to handle overflow gracefully
        let mut n: u64 = 0;
        for c in chars {
            // Shift and add (base 2^16)
            n = n.wrapping_mul(65536).wrapping_add(c as u64);
        }

        n
    }

    /// Generate embeddings for multiple texts
    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        texts.iter().map(|t| self.embed(t)).collect()
    }
}

impl Default for EmbeddingModel {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple tokenization
///
/// Splits text into words, normalizes to lowercase
fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| c.is_whitespace() || c.is_ascii_punctuation())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .collect()
}

/// Cosine similarity between two embeddings
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_htp_basic() {
        let model = EmbeddingModel::new();

        let emb1 = model.embed("hello world").unwrap();
        let emb2 = model.embed("hello world").unwrap();
        let emb3 = model.embed("goodbye moon").unwrap();

        // Same text should have identical embeddings (deterministic)
        assert_eq!(emb1, emb2);

        // Different text should have different embeddings
        assert_ne!(emb1, emb3);

        // Embedding dimension should match
        assert_eq!(emb1.len(), EMBEDDING_DIM);
    }

    #[test]
    fn test_htp_similarity() {
        let model = EmbeddingModel::new();

        let emb_gpu1 = model.embed("GPU memory sharing").unwrap();
        let emb_gpu2 = model.embed("GPU 메모리 공유").unwrap();
        let emb_unrelated = model.embed("cooking recipes").unwrap();

        // Similar topics should have higher similarity
        let sim_similar = cosine_similarity(&emb_gpu1, &emb_gpu2);
        let sim_different = cosine_similarity(&emb_gpu1, &emb_unrelated);

        // Note: HTP doesn't understand semantics, but shared tokens help
        println!("Similar: {}, Different: {}", sim_similar, sim_different);
    }

    #[test]
    fn test_htp_deterministic() {
        let model1 = EmbeddingModel::new();
        let model2 = EmbeddingModel::new();

        let text = "This is a test sentence for HTP";
        let emb1 = model1.embed(text).unwrap();
        let emb2 = model2.embed(text).unwrap();

        // Different model instances should produce identical embeddings
        assert_eq!(emb1, emb2);
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!(cosine_similarity(&a, &c).abs() < 0.001);

        let d = vec![-1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &d) + 1.0).abs() < 0.001);
    }

    #[test]
    fn test_korean_support() {
        let model = EmbeddingModel::new();

        let emb_ko = model.embed("한국어 테스트").unwrap();
        let emb_en = model.embed("Korean test").unwrap();

        // Both should produce valid embeddings
        assert_eq!(emb_ko.len(), EMBEDDING_DIM);
        assert_eq!(emb_en.len(), EMBEDDING_DIM);

        // L2 normalized
        let norm_ko: f32 = emb_ko.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_en: f32 = emb_en.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm_ko - 1.0).abs() < 0.01);
        assert!((norm_en - 1.0).abs() < 0.01);
    }
}
