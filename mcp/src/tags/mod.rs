//! Tag automation module for Elysium
//!
//! Provides semantic tag matching using Model2Vec embeddings.
//!
//! # Components
//!
//! - `embedder`: Model2Vec wrapper for text embeddings
//! - `database`: Tag database with descriptions and embeddings
//! - `matcher`: Tag suggestion logic
//! - `seeds`: Initial tag seed data

pub mod database;
pub mod embedder;
pub mod extractor;
pub mod keyword;
pub mod matcher;
pub mod seeds;

pub use database::{TagDatabase, TagEntry};
pub use embedder::TagEmbedder;
pub use extractor::{extract_tags_from_notes, ExtractResult};
pub use matcher::{TagMatcher, TagSuggestion};
pub use seeds::{seed_database, SEED_TAGS};
