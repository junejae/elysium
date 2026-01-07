# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0] - 2026-01-07

### Changed
- **BREAKING**: Removed folder structure management entirely
  - No longer creates/enforces Notes/, Projects/, Archive/ folders
  - Notes are scanned recursively from vault root (excluding dot-folders)
  - New notes created at vault root
  - Folder location is now 100% user's choice
- Simplified `.elysium.json` config - removed `folders` section
- Simplified `vault_status` output - shows total note count only
- **Plugin**: Major refactoring
  - ElysiumConfig: Added validation and migration support
  - Indexer: Improved error handling
  - MigrationEngine: v2 compatibility
  - IndexedDB storage initialization fixes
  - SetupWizard UI for first-time configuration

### Added
- `vault_create_note` MCP tool - creates note at vault root with frontmatter
- `vault_quick_capture` MCP tool - appends memo to inbox file
- `processing_guide` in `vault_get_inbox` response - helps AI process inbox items
- Quick Capture command in Obsidian plugin (`Cmd+Shift+N`)
- Quick Capture modal UI with text input
- Setup Wizard for plugin first-run experience

### Removed
- Folder-type validation in audit (notes can live anywhere)
- `FolderConfig` from configuration
- Folder mismatch checks from validation
- Folder counts from status output
- `elysium init --create` no longer creates folder structure

## [0.1.0] - 2025-01-06

### Added
- Initial release
- MCP server with semantic search using HTP embeddings
- SQLite-based vector storage
- YAML schema validation
- Wikilink integrity checking
- Vault health scoring
- Obsidian plugin with:
  - Status bar showing health score
  - Semantic search command
  - Tag cloud view
