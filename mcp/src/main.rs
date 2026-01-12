//! Elysium MCP Server
//!
//! MCP server for Obsidian-based Second Brain with AI-powered semantic search.
//! This binary starts the MCP server for Claude integration.

mod core;
mod mcp;
mod search;
mod tags;

fn main() -> anyhow::Result<()> {
    let vault_path = core::paths::get_vault_root();
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(mcp::run_mcp_server(vault_path))
}
