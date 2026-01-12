//! Model2Vec implementation for WASM
//!
//! This module provides Model2Vec embedding functionality that works in WASM environments.
//! It loads models from memory buffers instead of filesystem.

use half::f16;
use ndarray::Array2;
use safetensors::SafeTensors;
use serde::Deserialize;
use tokenizers::Tokenizer;

/// Model2Vec embedding dimension (potion-base-8M)
pub const EMBEDDING_DIM: usize = 256;

/// Model configuration from config.json
#[derive(Debug, Deserialize)]
pub struct ModelConfig {
    #[serde(default = "default_true")]
    pub normalize: bool,
    #[serde(default)]
    pub max_seq_length: Option<usize>,
}

fn default_true() -> bool {
    true
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            normalize: true,
            max_seq_length: None,
        }
    }
}

/// Model2Vec encoder that works in WASM
pub struct Model2Vec {
    embeddings: Array2<f32>,
    tokenizer: Tokenizer,
    normalize: bool,
}

impl Model2Vec {
    /// Load model from memory buffers
    ///
    /// # Arguments
    /// * `model_buffer` - Contents of model.safetensors
    /// * `tokenizer_buffer` - Contents of tokenizer.json
    /// * `config_buffer` - Contents of config.json
    pub fn from_buffers(
        model_buffer: &[u8],
        tokenizer_buffer: &[u8],
        config_buffer: &[u8],
    ) -> Result<Self, String> {
        // 1. Parse config.json
        let config: ModelConfig = serde_json::from_slice(config_buffer)
            .map_err(|e| format!("Failed to parse config.json: {}", e))?;

        // 2. Load tokenizer from bytes
        let tokenizer = Tokenizer::from_bytes(tokenizer_buffer)
            .map_err(|e| format!("Failed to load tokenizer: {}", e))?;

        // 3. Load embeddings from safetensors
        let tensors = SafeTensors::deserialize(model_buffer)
            .map_err(|e| format!("Failed to deserialize safetensors: {:?}", e))?;

        // Try different tensor names (model2vec uses "embeddings" or "0")
        let tensor = tensors
            .tensor("embeddings")
            .or_else(|_| tensors.tensor("0"))
            .map_err(|e| format!("No embeddings tensor found: {:?}", e))?;

        let embeddings = Self::tensor_to_array2(&tensor)?;

        Ok(Self {
            embeddings,
            tokenizer,
            normalize: config.normalize,
        })
    }

    /// Convert safetensors tensor to ndarray Array2<f32>
    fn tensor_to_array2(tensor: &safetensors::tensor::TensorView) -> Result<Array2<f32>, String> {
        let shape = tensor.shape();
        if shape.len() != 2 {
            return Err(format!("Expected 2D tensor, got {}D", shape.len()));
        }

        let rows = shape[0];
        let cols = shape[1];
        let data = tensor.data();

        // Handle different dtypes (f16 or f32)
        let float_data: Vec<f32> = match tensor.dtype() {
            safetensors::Dtype::F16 => {
                // Convert f16 bytes to f32
                data.chunks_exact(2)
                    .map(|chunk| {
                        let bits = u16::from_le_bytes([chunk[0], chunk[1]]);
                        f16::from_bits(bits).to_f32()
                    })
                    .collect()
            }
            safetensors::Dtype::F32 => {
                // Direct f32 bytes
                data.chunks_exact(4)
                    .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                    .collect()
            }
            safetensors::Dtype::BF16 => {
                // BFloat16 to f32
                data.chunks_exact(2)
                    .map(|chunk| {
                        let bits = u16::from_le_bytes([chunk[0], chunk[1]]);
                        // BF16 is just the upper 16 bits of f32
                        let f32_bits = (bits as u32) << 16;
                        f32::from_bits(f32_bits)
                    })
                    .collect()
            }
            dtype => return Err(format!("Unsupported dtype: {:?}", dtype)),
        };

        if float_data.len() != rows * cols {
            return Err(format!(
                "Data length mismatch: {} vs {}x{}={}",
                float_data.len(),
                rows,
                cols,
                rows * cols
            ));
        }

        Array2::from_shape_vec((rows, cols), float_data)
            .map_err(|e| format!("Failed to create array: {}", e))
    }

    /// Encode text to embedding vector
    pub fn encode(&self, text: &str) -> Vec<f32> {
        // 1. Tokenize
        let encoding = match self.tokenizer.encode(text, false) {
            Ok(enc) => enc,
            Err(_) => return vec![0.0; EMBEDDING_DIM],
        };

        let ids = encoding.get_ids();
        if ids.is_empty() {
            return vec![0.0; EMBEDDING_DIM];
        }

        // 2. Get embeddings for each token and compute mean
        let vocab_size = self.embeddings.nrows();
        let dim = self.embeddings.ncols();

        let mut sum = vec![0.0f64; dim];
        let mut count = 0usize;

        for &id in ids {
            let id = id as usize;
            if id < vocab_size {
                let row = self.embeddings.row(id);
                for (i, &val) in row.iter().enumerate() {
                    sum[i] += val as f64;
                }
                count += 1;
            }
        }

        if count == 0 {
            return vec![0.0; EMBEDDING_DIM];
        }

        // 3. Average
        let mut result: Vec<f32> = sum.iter().map(|v| (*v / count as f64) as f32).collect();

        // 4. Normalize if configured
        if self.normalize {
            let norm: f32 = result.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 1e-12 {
                for v in &mut result {
                    *v /= norm;
                }
            }
        }

        result
    }

    /// Get the embedding dimension
    pub fn dim(&self) -> usize {
        self.embeddings.ncols()
    }

    /// Get vocabulary size
    pub fn vocab_size(&self) -> usize {
        self.embeddings.nrows()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_config_default() {
        let config = ModelConfig::default();
        assert!(config.normalize);
    }
}
