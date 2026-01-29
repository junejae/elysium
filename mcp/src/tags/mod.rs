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

#[allow(unused_imports)]
pub use database::{TagDatabase, TagEntry};
#[allow(unused_imports)]
pub use embedder::TagEmbedder;
#[allow(unused_imports)]
pub use extractor::{extract_tags_from_notes, ExtractResult};
#[allow(unused_imports)]
pub use matcher::{TagMatcher, TagSuggestion};
#[allow(unused_imports)]
pub use seeds::{seed_database, SEED_TAGS};
