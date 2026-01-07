use serde::{Deserialize, Serialize};
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::cmp::Ordering;

const M: usize = 16;
const M_MAX: usize = M;
const M_MAX_0: usize = M * 2;
const EF_CONSTRUCTION: usize = 200;

fn ml_factor() -> f64 {
    1.0 / (M as f64).ln()
}

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
        other.distance.partial_cmp(&self.distance).unwrap_or(Ordering::Equal)
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
        self.distance.partial_cmp(&other.distance).unwrap_or(Ordering::Equal)
    }
}

impl Default for HnswIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl HnswIndex {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            entry_point: None,
            max_level: 0,
            id_to_idx: HashMap::new(),
            deleted: HashSet::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.nodes.len() - self.deleted.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn random_level() -> usize {
        let r: f64 = rand::random();
        (-r.ln() * ml_factor()).floor() as usize
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

    pub fn insert(&mut self, id: String, vector: Vec<f32>) {
        if let Some(&existing_idx) = self.id_to_idx.get(&id) {
            self.nodes[existing_idx].vector = vector;
            return;
        }

        let level = Self::random_level();
        let node_idx = self.nodes.len();

        let mut neighbors = Vec::with_capacity(level + 1);
        for _ in 0..=level {
            neighbors.push(Vec::new());
        }

        self.nodes.push(Node {
            id: id.clone(),
            vector,
            level,
            neighbors,
        });
        self.id_to_idx.insert(id, node_idx);

        if self.entry_point.is_none() {
            self.entry_point = Some(node_idx);
            self.max_level = level;
            return;
        }

        let query = self.nodes[node_idx].vector.clone();
        let mut ep = self.entry_point.unwrap();

        for lc in (level + 1..=self.max_level).rev() {
            ep = self.search_layer_single(&query, ep, lc);
        }

        for lc in (0..=level.min(self.max_level)).rev() {
            let m_max = if lc == 0 { M_MAX_0 } else { M_MAX };
            let neighbors_at_level = self.search_layer(&query, ep, EF_CONSTRUCTION, lc);
            let selected = self.select_neighbors(&neighbors_at_level, m_max);

            self.nodes[node_idx].neighbors[lc] = selected.clone();

            for &neighbor_idx in &selected {
                if self.deleted.contains(&neighbor_idx) {
                    continue;
                }
                let neighbor_level = self.nodes[neighbor_idx].level;
                if lc <= neighbor_level {
                    self.nodes[neighbor_idx].neighbors[lc].push(node_idx);
                    if self.nodes[neighbor_idx].neighbors[lc].len() > m_max {
                        let neighbor_vec = self.nodes[neighbor_idx].vector.clone();
                        let old_neighbors: Vec<usize> =
                            self.nodes[neighbor_idx].neighbors[lc].clone();
                        let candidates: Vec<Candidate> = old_neighbors
                            .iter()
                            .filter(|&&n| !self.deleted.contains(&n))
                            .map(|&n| Candidate {
                                idx: n,
                                distance: Self::distance(&neighbor_vec, &self.nodes[n].vector),
                            })
                            .collect();
                        let pruned = self.select_neighbors_from_candidates(&candidates, m_max);
                        self.nodes[neighbor_idx].neighbors[lc] = pruned;
                    }
                }
            }

            if !selected.is_empty() {
                ep = selected[0];
            }
        }

        if level > self.max_level {
            self.max_level = level;
            self.entry_point = Some(node_idx);
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

    fn search_layer(&self, query: &[f32], ep: usize, ef: usize, level: usize) -> Vec<Candidate> {
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

        while let Some(Candidate { idx: c_idx, distance: c_dist }) = candidates.pop() {
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

        results
            .into_sorted_vec()
            .into_iter()
            .map(|fc| Candidate {
                idx: fc.idx,
                distance: fc.distance,
            })
            .collect()
    }

    fn select_neighbors(&self, candidates: &[Candidate], m: usize) -> Vec<usize> {
        self.select_neighbors_from_candidates(candidates, m)
    }

    fn select_neighbors_from_candidates(&self, candidates: &[Candidate], m: usize) -> Vec<usize> {
        let mut sorted: Vec<_> = candidates.iter().cloned().collect();
        sorted.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(Ordering::Equal));
        sorted.into_iter().take(m).map(|c| c.idx).collect()
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
            .filter(|c| !self.deleted.contains(&c.idx))
            .take(k)
            .map(|c| {
                let similarity = 1.0 - c.distance;
                (self.nodes[c.idx].id.clone(), similarity)
            })
            .collect()
    }

    pub fn delete(&mut self, id: &str) -> bool {
        if let Some(&idx) = self.id_to_idx.get(id) {
            self.deleted.insert(idx);
            true
        } else {
            false
        }
    }

    pub fn contains(&self, id: &str) -> bool {
        if let Some(&idx) = self.id_to_idx.get(id) {
            !self.deleted.contains(&idx)
        } else {
            false
        }
    }

    pub fn get_vector(&self, id: &str) -> Option<Vec<f32>> {
        self.id_to_idx.get(id).and_then(|&idx| {
            if self.deleted.contains(&idx) {
                None
            } else {
                Some(self.nodes[idx].vector.clone())
            }
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap_or_default()
    }

    pub fn deserialize(data: &[u8]) -> Option<Self> {
        bincode::deserialize(data).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn random_vector(dim: usize) -> Vec<f32> {
        (0..dim).map(|_| rand::random::<f32>()).collect()
    }

    #[test]
    fn test_insert_and_search() {
        let mut index = HnswIndex::new();
        let dim = 384;

        for i in 0..100 {
            let vec = random_vector(dim);
            index.insert(format!("doc_{}", i), vec);
        }

        assert_eq!(index.len(), 100);

        let query = random_vector(dim);
        let results = index.search(&query, 10, 50);
        assert_eq!(results.len(), 10);
    }

    #[test]
    fn test_delete() {
        let mut index = HnswIndex::new();
        index.insert("doc_1".to_string(), vec![1.0, 0.0, 0.0]);
        index.insert("doc_2".to_string(), vec![0.0, 1.0, 0.0]);

        assert_eq!(index.len(), 2);
        assert!(index.contains("doc_1"));

        index.delete("doc_1");
        assert_eq!(index.len(), 1);
        assert!(!index.contains("doc_1"));
    }

    #[test]
    fn test_similarity_ranking() {
        let mut index = HnswIndex::new();

        index.insert("similar".to_string(), vec![1.0, 0.0, 0.0]);
        index.insert("different".to_string(), vec![0.0, 1.0, 0.0]);

        let query = vec![0.9, 0.1, 0.0];
        let results = index.search(&query, 2, 10);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "similar");
    }

    #[test]
    fn test_serialization() {
        let mut index = HnswIndex::new();
        index.insert("doc_1".to_string(), vec![1.0, 0.0, 0.0]);
        index.insert("doc_2".to_string(), vec![0.0, 1.0, 0.0]);

        let serialized = index.serialize();
        let restored = HnswIndex::deserialize(&serialized).unwrap();

        assert_eq!(restored.len(), 2);
        assert!(restored.contains("doc_1"));
        assert!(restored.contains("doc_2"));
    }
}
