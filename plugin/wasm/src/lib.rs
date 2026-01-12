mod hnsw;
mod model2vec;

use wasm_bindgen::prelude::*;
use std::f64::consts::PI;

/// HTP embedding dimension
const EMBEDDING_DIM: usize = 384;
/// Model2Vec embedding dimension
pub const MODEL2VEC_DIM: usize = 256;
const NUM_MODULI: usize = EMBEDDING_DIM / 2;

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

#[wasm_bindgen]
pub fn embed_text(text: &str) -> Vec<f32> {
    let tokens = tokenize(text);
    
    if tokens.is_empty() {
        return vec![0.0; EMBEDDING_DIM];
    }

    let mut sum_embedding = vec![0.0f64; EMBEDDING_DIM];
    
    for token in &tokens {
        let token_emb = embed_token(token);
        for (i, val) in token_emb.iter().enumerate() {
            sum_embedding[i] += val;
        }
    }

    let count = tokens.len() as f64;
    for val in &mut sum_embedding {
        *val /= count;
    }

    let norm: f64 = sum_embedding.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm > 0.0 {
        sum_embedding.iter().map(|x| (*x / norm) as f32).collect()
    } else {
        sum_embedding.iter().map(|x| *x as f32).collect()
    }
}

fn embed_token(token: &str) -> Vec<f64> {
    let n = token_to_integer(token);
    let mut embedding = Vec::with_capacity(EMBEDDING_DIM);

    for &m in COPRIME_MODULI.iter().take(NUM_MODULI) {
        let r = n % m;
        let theta = 2.0 * PI * (r as f64) / (m as f64);
        embedding.push(theta.sin());
        embedding.push(theta.cos());
    }

    embedding
}

fn token_to_integer(token: &str) -> u64 {
    let mut n: u64 = 0;
    for c in token.chars().take(64) {
        n = n.wrapping_mul(65536).wrapping_add(c as u64);
    }
    n
}

fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| c.is_whitespace() || c.is_ascii_punctuation())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .collect()
}

#[wasm_bindgen]
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

#[wasm_bindgen]
pub fn get_embedding_dim() -> usize {
    EMBEDDING_DIM
}

#[wasm_bindgen]
pub struct HnswIndex {
    inner: hnsw::HnswIndex,
}

#[wasm_bindgen]
impl HnswIndex {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: hnsw::HnswIndex::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn insert(&mut self, id: &str, vector: Vec<f32>) {
        self.inner.insert(id.to_string(), vector);
    }

    pub fn insert_text(&mut self, id: &str, text: &str) {
        let vector = embed_text(text);
        self.inner.insert(id.to_string(), vector);
    }

    pub fn search(&self, query: &[f32], k: usize, ef: usize) -> JsValue {
        let results = self.inner.search(query, k, ef);
        serde_wasm_bindgen::to_value(&results).unwrap_or(JsValue::NULL)
    }

    pub fn search_text(&self, text: &str, k: usize, ef: usize) -> JsValue {
        let query = embed_text(text);
        self.search(&query, k, ef)
    }

    pub fn delete(&mut self, id: &str) -> bool {
        self.inner.delete(id)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.inner.contains(id)
    }

    pub fn get_vector(&self, id: &str) -> Option<Vec<f32>> {
        self.inner.get_vector(id)
    }

    pub fn serialize(&self) -> Vec<u8> {
        self.inner.serialize()
    }

    pub fn deserialize(data: &[u8]) -> Option<HnswIndex> {
        hnsw::HnswIndex::deserialize(data).map(|inner| Self { inner })
    }
}

impl Default for HnswIndex {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Model2Vec Encoder (256D neural network embeddings)
// ============================================================================

/// Model2Vec encoder for advanced semantic search
///
/// Usage from JavaScript:
/// ```js
/// const encoder = new Model2VecEncoder();
/// await encoder.load(modelBytes, tokenizerBytes, configBytes);
/// const embedding = encoder.encode("hello world");
/// ```
#[wasm_bindgen]
pub struct Model2VecEncoder {
    inner: Option<model2vec::Model2Vec>,
}

#[wasm_bindgen]
impl Model2VecEncoder {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self { inner: None }
    }

    /// Load model from memory buffers
    ///
    /// # Arguments
    /// * `model_buffer` - Contents of model.safetensors
    /// * `tokenizer_buffer` - Contents of tokenizer.json
    /// * `config_buffer` - Contents of config.json
    pub fn load(
        &mut self,
        model_buffer: &[u8],
        tokenizer_buffer: &[u8],
        config_buffer: &[u8],
    ) -> Result<(), JsValue> {
        let model = model2vec::Model2Vec::from_buffers(
            model_buffer,
            tokenizer_buffer,
            config_buffer,
        )
        .map_err(|e| JsValue::from_str(&e))?;

        self.inner = Some(model);
        Ok(())
    }

    /// Encode text to 256D embedding vector
    pub fn encode(&self, text: &str) -> Result<Vec<f32>, JsValue> {
        match &self.inner {
            Some(model) => Ok(model.encode(text)),
            None => Err(JsValue::from_str("Model not loaded. Call load() first.")),
        }
    }

    /// Check if model is loaded
    pub fn is_loaded(&self) -> bool {
        self.inner.is_some()
    }

    /// Get embedding dimension (256 for potion-base-8M)
    pub fn dim(&self) -> usize {
        self.inner.as_ref().map(|m| m.dim()).unwrap_or(MODEL2VEC_DIM)
    }

    /// Get vocabulary size
    pub fn vocab_size(&self) -> usize {
        self.inner.as_ref().map(|m| m.vocab_size()).unwrap_or(0)
    }
}

impl Default for Model2VecEncoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Get Model2Vec embedding dimension
#[wasm_bindgen]
pub fn get_model2vec_dim() -> usize {
    MODEL2VEC_DIM
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embed_deterministic() {
        let emb1 = embed_text("hello world");
        let emb2 = embed_text("hello world");
        assert_eq!(emb1, emb2);
    }

    #[test]
    fn test_embed_dimension() {
        let emb = embed_text("test");
        assert_eq!(emb.len(), EMBEDDING_DIM);
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let emb = embed_text("test");
        let sim = cosine_similarity(&emb, &emb);
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_hnsw_basic() {
        let mut index = HnswIndex::new();
        
        index.insert_text("doc1", "GPU memory optimization");
        index.insert_text("doc2", "cooking recipes");
        index.insert_text("doc3", "CUDA kernel programming");
        
        assert_eq!(index.len(), 3);
        assert!(index.contains("doc1"));
        assert!(!index.contains("doc4"));
    }
}
