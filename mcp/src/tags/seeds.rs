//! Tag seed data for initial database population
//!
//! Core tags with descriptions for semantic matching.

use anyhow::Result;

use super::database::TagDatabase;
use super::embedder::TagEmbedder;

/// Seed tag definition
pub struct SeedTag {
    pub name: &'static str,
    pub description: &'static str,
    pub aliases: &'static [&'static str],
}

/// Core tags for the vault
pub const SEED_TAGS: &[SeedTag] = &[
    // === Tech (area: tech) ===
    SeedTag {
        name: "gpu",
        description: "GPU hardware, VRAM, graphics card, memory optimization, parallel computing",
        aliases: &["vram", "graphics_card"],
    },
    SeedTag {
        name: "cuda",
        description: "NVIDIA CUDA programming, GPU computing, cuDNN, CUDA toolkit",
        aliases: &["nvidia_cuda"],
    },
    SeedTag {
        name: "mps",
        description: "Apple Metal Performance Shaders, M1/M2 GPU acceleration",
        aliases: &["metal"],
    },
    SeedTag {
        name: "mig",
        description: "NVIDIA Multi-Instance GPU, GPU partitioning, A100 MIG",
        aliases: &[],
    },
    SeedTag {
        name: "nvlink",
        description: "NVIDIA NVLink, GPU interconnect, multi-GPU communication",
        aliases: &[],
    },
    SeedTag {
        name: "time_slicing",
        description: "GPU time slicing, temporal multiplexing, GPU sharing",
        aliases: &[],
    },
    SeedTag {
        name: "k8s",
        description: "Kubernetes, container orchestration, cluster management, pods, deployments",
        aliases: &["kubernetes"],
    },
    SeedTag {
        name: "inference",
        description: "Model inference, serving, deployment, optimization, latency",
        aliases: &["model_serving"],
    },
    SeedTag {
        name: "llm",
        description: "Large language models, GPT, Claude, LLaMA, transformers, 대규모 언어 모델",
        aliases: &["large_language_model", "gpt"],
    },
    SeedTag {
        name: "ml_ops",
        description: "MLOps, machine learning operations, model lifecycle, deployment pipelines",
        aliases: &["mlops"],
    },
    SeedTag {
        name: "distributed",
        description: "Distributed computing, parallel processing, multi-node training",
        aliases: &[],
    },
    SeedTag {
        name: "aws",
        description: "Amazon Web Services, cloud computing, EC2, S3, Lambda",
        aliases: &["amazon"],
    },
    SeedTag {
        name: "gcp",
        description: "Google Cloud Platform, cloud computing, GKE, BigQuery",
        aliases: &["google_cloud"],
    },
    SeedTag {
        name: "agents",
        description: "AI agents, autonomous systems, tool use, agentic workflows",
        aliases: &["ai_agents"],
    },
    SeedTag {
        name: "mcp",
        description: "Model Context Protocol, AI tool integration, Claude Code",
        aliases: &["model_context_protocol"],
    },
    SeedTag {
        name: "elysium",
        description: "Elysium vault system, second brain, knowledge management",
        aliases: &[],
    },
    SeedTag {
        name: "embedding",
        description: "Text embeddings, vector representations, semantic similarity, 임베딩",
        aliases: &["embeddings"],
    },
    SeedTag {
        name: "rag",
        description: "Retrieval Augmented Generation, knowledge retrieval, context augmentation",
        aliases: &[],
    },
    SeedTag {
        name: "deep_learning",
        description: "Deep learning, neural networks, backpropagation, 딥러닝",
        aliases: &["dl"],
    },
    SeedTag {
        name: "transformer",
        description: "Transformer architecture, attention mechanism, BERT, GPT",
        aliases: &[],
    },
    SeedTag {
        name: "architecture",
        description: "System architecture, software design, infrastructure planning",
        aliases: &[],
    },
    SeedTag {
        name: "search",
        description: "Search systems, information retrieval, semantic search, 검색",
        aliases: &[],
    },
    SeedTag {
        name: "obsidian",
        description: "Obsidian app, note-taking, knowledge management, plugins",
        aliases: &[],
    },
    // === Work (area: work) ===
    SeedTag {
        name: "gs_neotek",
        description: "GS네오텍 company, work-related, 회사",
        aliases: &["네오텍", "neotek"],
    },
    SeedTag {
        name: "meeting",
        description: "Meeting notes, discussions, decisions, 회의",
        aliases: &["회의"],
    },
    SeedTag {
        name: "onboarding",
        description: "Onboarding process, new employee, training",
        aliases: &[],
    },
    // === Life (area: life) ===
    SeedTag {
        name: "health",
        description: "Health, wellness, medical, exercise, 건강",
        aliases: &["건강"],
    },
    SeedTag {
        name: "finance",
        description: "Personal finance, investment, budgeting, 재정",
        aliases: &["재정"],
    },
    SeedTag {
        name: "routine",
        description: "Daily routines, habits, productivity systems",
        aliases: &[],
    },
    // === Career (area: career) ===
    SeedTag {
        name: "portfolio",
        description: "Portfolio, projects showcase, career highlights",
        aliases: &[],
    },
    SeedTag {
        name: "resume",
        description: "Resume, CV, career history, job applications",
        aliases: &["cv"],
    },
];

/// Initialize tag database with seed tags
pub fn seed_database(db: &TagDatabase, embedder: &TagEmbedder) -> Result<usize> {
    let mut count = 0;

    for seed in SEED_TAGS {
        // Skip if tag already exists
        if db.get_tag(seed.name)?.is_some() {
            continue;
        }

        // Add tag with embedding
        let tag_id = db.add_tag(seed.name, seed.description, embedder)?;

        // Add aliases
        for alias in seed.aliases {
            db.add_alias(seed.name, alias)?;
        }

        count += 1;
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_seed_count() {
        assert!(SEED_TAGS.len() >= 30, "Should have at least 30 seed tags");
    }

    #[test]
    fn test_seed_unique_names() {
        let mut names: Vec<_> = SEED_TAGS.iter().map(|t| t.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), SEED_TAGS.len(), "Tag names should be unique");
    }
}
