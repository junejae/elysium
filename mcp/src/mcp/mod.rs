//! MCP Server for Second Brain Vault
//!
//! Provides AI-native access to vault search and note operations.

mod audit;
mod helpers;
mod params;
mod server;
mod types;

pub use server::run_mcp_server;
