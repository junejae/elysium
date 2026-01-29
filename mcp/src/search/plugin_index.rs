//! Plugin Index Reader - Read and search Obsidian plugin's exported index
//!
//! This module reads the index files exported by the Elysium Obsidian plugin:
//! - hnsw.bin: HNSW vector index (bincode serialized)
//! - notes.json: Note metadata (path, gist, fields, tags)
//! - meta.json: Index metadata (embedding mode, dimension, timestamp)

use anyhow::{Context, Result};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::embedder::{create_embedder, SearchConfig};

// ============================================================================
// HNSW Index (copied from plugin WASM for binary compatibility)
// ============================================================================

pub(crate) const PLUGIN_INDEX_VERSION: u32 = 1;

#[derive(Clone, Serialize, Deserialize)]
pub struct HnswIndex {
    nodes: Vec<Node>,
    entry_point: Option<usize>,
    max_level: usize,
    id_to_idx: HashMap<String, usize>,
    deleted: HashSet<usize>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Node {
    id: String,
    vector: Vec<f32>,
    level: usize,
    neighbors: Vec<Vec<usize>>,
}

impl HnswIndex {
    pub fn deserialize(data: &[u8]) -> Option<Self> {
        bincode::deserialize(data).ok()
    }

    pub fn len(&self) -> usize {
        self.nodes.len() - self.deleted.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn distance(a: &[f32], b: &[f32]) -> f32 {
        1.0 - Self::cosine_similarity(a, b)
    }

    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
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

    fn search_layer_single(&self, query: &[f32], ep: usize, level: usize) -> usize {
        let mut current = ep;
        let mut current_dist = Self::distance(query, &self.nodes[current].vector);

        loop {
            let mut changed = false;
            if level < self.nodes[current].neighbors.len() {
                for &neighbor in &self.nodes[current].neighbors[level] {
                    if self.deleted.contains(&neighbor) {
                        continue;
                    }
                    let dist = Self::distance(query, &self.nodes[neighbor].vector);
                    if dist < current_dist {
                        current = neighbor;
                        current_dist = dist;
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        current
    }

    fn search_layer(&self, query: &[f32], ep: usize, ef: usize, level: usize) -> Vec<(usize, f32)> {
        use std::cmp::Ordering;
        use std::collections::BinaryHeap;

        #[derive(Clone, Copy)]
        struct Candidate {
            idx: usize,
            distance: f32,
        }

        impl PartialEq for Candidate {
            fn eq(&self, other: &Self) -> bool {
                self.distance == other.distance
            }
        }
        impl Eq for Candidate {}
        impl PartialOrd for Candidate {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }
        impl Ord for Candidate {
            fn cmp(&self, other: &Self) -> Ordering {
                other
                    .distance
                    .partial_cmp(&self.distance)
                    .unwrap_or(Ordering::Equal)
            }
        }

        #[derive(Clone, Copy)]
        struct FarCandidate {
            idx: usize,
            distance: f32,
        }

        impl PartialEq for FarCandidate {
            fn eq(&self, other: &Self) -> bool {
                self.distance == other.distance
            }
        }
        impl Eq for FarCandidate {}
        impl PartialOrd for FarCandidate {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }
        impl Ord for FarCandidate {
            fn cmp(&self, other: &Self) -> Ordering {
                self.distance
                    .partial_cmp(&other.distance)
                    .unwrap_or(Ordering::Equal)
            }
        }

        let mut visited = HashSet::new();
        let mut candidates = BinaryHeap::new();
        let mut results = BinaryHeap::new();

        let dist = Self::distance(query, &self.nodes[ep].vector);
        visited.insert(ep);
        candidates.push(Candidate {
            idx: ep,
            distance: dist,
        });
        results.push(FarCandidate {
            idx: ep,
            distance: dist,
        });

        while let Some(Candidate {
            idx: c_idx,
            distance: c_dist,
        }) = candidates.pop()
        {
            let worst_dist = results.peek().map(|r| r.distance).unwrap_or(f32::MAX);
            if c_dist > worst_dist && results.len() >= ef {
                break;
            }

            if level < self.nodes[c_idx].neighbors.len() {
                for &neighbor in &self.nodes[c_idx].neighbors[level] {
                    if visited.contains(&neighbor) || self.deleted.contains(&neighbor) {
                        continue;
                    }
                    visited.insert(neighbor);

                    let dist = Self::distance(query, &self.nodes[neighbor].vector);
                    let worst = results.peek().map(|r| r.distance).unwrap_or(f32::MAX);

                    if dist < worst || results.len() < ef {
                        candidates.push(Candidate {
                            idx: neighbor,
                            distance: dist,
                        });
                        results.push(FarCandidate {
                            idx: neighbor,
                            distance: dist,
                        });
                        if results.len() > ef {
                            results.pop();
                        }
                    }
                }
            }
        }

        let mut sorted: Vec<_> = results.into_iter().collect();
        sorted.sort_by(|a, b| {
            a.distance
                .partial_cmp(&b.distance)
                .unwrap_or(Ordering::Equal)
        });
        sorted.into_iter().map(|fc| (fc.idx, fc.distance)).collect()
    }

    pub fn search(&self, query: &[f32], k: usize, ef: usize) -> Vec<(String, f32)> {
        if self.entry_point.is_none() || self.is_empty() {
            return Vec::new();
        }

        let mut ep = self.entry_point.unwrap();

        for lc in (1..=self.max_level).rev() {
            ep = self.search_layer_single(query, ep, lc);
        }

        let candidates = self.search_layer(query, ep, ef.max(k), 0);

        candidates
            .into_iter()
            .filter(|(idx, _)| !self.deleted.contains(idx))
            .take(k)
            .map(|(idx, distance)| {
                let similarity = 1.0 - distance;
                (self.nodes[idx].id.clone(), similarity)
            })
            .collect()
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn from_vectors(ids: Vec<String>, vectors: Vec<Vec<f32>>) -> Self {
        assert_eq!(
            ids.len(),
            vectors.len(),
            "ids and vectors must be same length"
        );
        let mut id_to_idx = HashMap::new();
        for (idx, id) in ids.iter().enumerate() {
            id_to_idx.insert(id.clone(), idx);
        }

        let node_count = ids.len();
        let nodes = ids
            .into_iter()
            .zip(vectors.into_iter())
            .enumerate()
            .map(|(idx, (id, vector))| {
                let neighbors = (0..node_count)
                    .filter(|neighbor| *neighbor != idx)
                    .collect::<Vec<_>>();
                Node {
                    id,
                    vector,
                    level: 0,
                    neighbors: vec![neighbors],
                }
            })
            .collect::<Vec<_>>();

        Self {
            nodes,
            entry_point: if node_count > 0 { Some(0) } else { None },
            max_level: 0,
            id_to_idx,
            deleted: HashSet::new(),
        }
    }
}

// ============================================================================
// Plugin Index Metadata
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexMeta {
    pub embedding_mode: String,
    pub dimension: usize,
    pub note_count: usize,
    pub index_size: usize,
    pub exported_at: u64,
    pub version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteRecord {
    pub path: String,
    pub gist: String,
    pub mtime: u64,
    pub indexed: bool,
    #[serde(default, deserialize_with = "deserialize_fields")]
    pub fields: HashMap<String, String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

fn deserialize_fields<'de, D>(deserializer: D) -> Result<HashMap<String, String>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde_json::Value;

    let raw = HashMap::<String, Value>::deserialize(deserializer)?;
    let mut fields = HashMap::new();

    for (key, value) in raw {
        match value {
            Value::String(s) => {
                fields.insert(key, s);
            }
            Value::Number(n) => {
                fields.insert(key, n.to_string());
            }
            Value::Bool(b) => {
                fields.insert(key, b.to_string());
            }
            Value::Array(items) => {
                let joined = items
                    .iter()
                    .filter_map(|item| item.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                if !joined.is_empty() {
                    fields.insert(key, joined);
                }
            }
            _ => {}
        }
    }

    Ok(fields)
}

// ============================================================================
// Plugin Index Reader
// ============================================================================

pub struct PluginIndexReader {
    index_dir: std::path::PathBuf,
}

impl PluginIndexReader {
    pub fn new(vault_path: &Path) -> Self {
        let index_dir = vault_path.join(".obsidian/plugins/elysium/index");
        Self { index_dir }
    }

    pub fn exists(&self) -> bool {
        self.index_dir.join("hnsw.bin").exists()
            && self.index_dir.join("notes.json").exists()
            && self.index_dir.join("meta.json").exists()
    }

    pub fn load_meta(&self) -> Result<IndexMeta> {
        let meta_path = self.index_dir.join("meta.json");
        let content = std::fs::read_to_string(&meta_path)
            .with_context(|| format!("Failed to read meta.json from {:?}", meta_path))?;
        serde_json::from_str(&content).context("Failed to parse meta.json")
    }

    pub fn load_notes(&self) -> Result<Vec<NoteRecord>> {
        let notes_path = self.index_dir.join("notes.json");
        let content = std::fs::read_to_string(&notes_path)
            .with_context(|| format!("Failed to read notes.json from {:?}", notes_path))?;
        serde_json::from_str(&content).context("Failed to parse notes.json")
    }

    pub fn load_hnsw(&self) -> Result<HnswIndex> {
        let hnsw_path = self.index_dir.join("hnsw.bin");
        let data = std::fs::read(&hnsw_path)
            .with_context(|| format!("Failed to read hnsw.bin from {:?}", hnsw_path))?;
        HnswIndex::deserialize(&data).context("Failed to deserialize HNSW index")
    }
}

// ============================================================================
// Plugin Search Engine
// ============================================================================

use super::embedder::Embedder;
use super::engine::SearchResult;

pub struct PluginSearchEngine {
    hnsw: HnswIndex,
    notes: HashMap<String, NoteRecord>,
    embedder: Box<dyn Embedder>,
    #[allow(dead_code)]
    meta: IndexMeta,
}

impl PluginSearchEngine {
    pub fn load(vault_path: &Path) -> Result<Self> {
        let reader = PluginIndexReader::new(vault_path);

        if !reader.exists() {
            anyhow::bail!(
                "Plugin index not found. Please enable indexing in Obsidian Elysium plugin."
            );
        }

        let meta = reader.load_meta()?;
        if meta.version != PLUGIN_INDEX_VERSION {
            anyhow::bail!(
                "Plugin index version mismatch (expected {}, found {}). Rebuild the index with a compatible plugin.",
                PLUGIN_INDEX_VERSION,
                meta.version
            );
        }
        let notes_vec = reader.load_notes()?;
        let hnsw = reader.load_hnsw()?;

        // Create embedder matching plugin's embedding mode
        // Use model downloaded by plugin if advanced search is enabled
        let model_path = if meta.embedding_mode == "model2vec" {
            let plugin_model_path =
                vault_path.join(".obsidian/plugins/elysium/models/potion-multilingual-128M");
            if plugin_model_path.exists() {
                Some(plugin_model_path.to_string_lossy().to_string())
            } else {
                None
            }
        } else {
            None
        };

        let search_config = SearchConfig {
            use_advanced: meta.embedding_mode == "model2vec",
            model_path,
            model_id: None,
        };
        let embedder = create_embedder(&search_config)?;

        // Verify dimension matches
        if embedder.dimension() != meta.dimension {
            anyhow::bail!(
                "Embedding dimension mismatch: embedder={}, index={}. Mode: {}",
                embedder.dimension(),
                meta.dimension,
                meta.embedding_mode
            );
        }

        // Build notes lookup
        let notes: HashMap<String, NoteRecord> =
            notes_vec.into_iter().map(|n| (n.path.clone(), n)).collect();

        Ok(Self {
            hnsw,
            notes,
            embedder,
            meta,
        })
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let query_embedding = self.embedder.embed(query)?;
        let results = self.hnsw.search(&query_embedding, limit, 50);

        Ok(results
            .into_iter()
            .filter_map(|(path, score)| {
                let note = self.notes.get(&path)?;
                Some(SearchResult {
                    id: path.clone(),
                    path,
                    title: note
                        .path
                        .rsplit('/')
                        .next()
                        .unwrap_or(&note.path)
                        .trim_end_matches(".md")
                        .to_string(),
                    gist: Some(note.gist.clone()),
                    note_type: note.fields.get("type").cloned(),
                    area: note.fields.get("area").cloned(),
                    score,
                })
            })
            .collect())
    }

    pub fn get_note(&self, path: &str) -> Option<&NoteRecord> {
        self.notes.get(path)
    }

    /// Iterate over all note records
    pub fn iter_notes(&self) -> impl Iterator<Item = &NoteRecord> {
        self.notes.values()
    }

    #[allow(dead_code)]
    pub fn note_count(&self) -> usize {
        self.notes.len()
    }

    #[allow(dead_code)]
    pub fn embedding_mode(&self) -> &str {
        &self.meta.embedding_mode
    }

    #[allow(dead_code)]
    pub fn dimension(&self) -> usize {
        self.meta.dimension
    }

    #[allow(dead_code)]
    pub fn exported_at(&self) -> u64 {
        self.meta.exported_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hnsw_deserialize_format() {
        // Test that our HNSW struct matches the plugin's serialization format
        // This is a minimal test - actual integration testing requires plugin index files
        let index = HnswIndex {
            nodes: vec![],
            entry_point: None,
            max_level: 0,
            id_to_idx: HashMap::new(),
            deleted: HashSet::new(),
        };

        let serialized = bincode::serialize(&index).unwrap();
        let deserialized = HnswIndex::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.len(), 0);
        assert!(deserialized.is_empty());
    }
}
