# Contracts

This folder captures the MCP ↔ plugin index export contract.

## Current Version
- **Plugin Index Contract**: v1
  - `meta.json` schema: `plugin-index-meta.schema.json`
  - `notes.json` schema: `plugin-index-notes.schema.json`

## Files
- `.obsidian/plugins/elysium/index/meta.json`
- `.obsidian/plugins/elysium/index/notes.json`
- `.obsidian/plugins/elysium/index/hnsw.bin` (binary, not covered by JSON schema)

## Compatibility
- MCP expects `meta.json.version == 1`.
- If the version changes, update the schema files and bump the expected version in both:
  - `plugin/src/indexer/Indexer.ts`
  - `mcp/src/search/plugin_index.rs`

## Enforcement
- Plugin export validates the contract before writing index files.
- MCP load rejects incompatible versions with a clear error.
