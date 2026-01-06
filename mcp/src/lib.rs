//! vault-tools library
//!
//! Second Brain Vault CLI tools and search engine.
//!
//! # Modules
//!
//! - `core`: Core vault operations (notes, frontmatter, wikilinks)
//! - `search`: Semantic search engine (Phase 1+)
//! - `mcp`: MCP server for Claude integration (Phase 1+)

pub mod core;
pub mod search;

// Re-exports for convenience
pub use core::frontmatter::Frontmatter;
pub use core::note::{collect_all_notes, collect_note_names, Note};
pub use core::paths::{get_vault_root, VaultPaths, VAULT_PATH_ENV};
pub use core::schema::{SchemaViolation, VALID_AREAS, VALID_STATUS, VALID_TYPES};
pub use core::wikilink::extract_wikilinks;
