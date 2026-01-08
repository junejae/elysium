# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.6.2] - 2026-01-08

### Fixed
- **CI**: Binary version mismatch - built binaries now correctly report version from git tag
  - Added `Sync Cargo.toml version from tag` step in release workflow
  - Cargo.toml version is updated before build, ensuring `--version` output matches release
- **CI**: Windows build failure - added PowerShell-compatible version sync script

## [0.6.0] - 2026-01-08

### Added
- **MCP**: `vault_save` - Unified save interface with strategy-based saving
  - `strategy: create` - Create new note with frontmatter
  - `strategy: update` - Overwrite existing note
  - `strategy: append` - Add content to existing note
  - `strategy: inbox` - Quick capture to inbox.md
  - `strategy: smart` - Auto-detect duplicates using semantic search
- **MCP**: `source` field support in frontmatter for web research notes
- **MCP**: Similarity threshold parameter for smart save (default: 0.7)

### Changed
- **BREAKING**: Removed `vault_create_note` - use `vault_save(strategy="create")`
- **BREAKING**: Removed `vault_quick_capture` - use `vault_save(strategy="inbox")`
- **BREAKING**: Removed `vault_update_gist` - use `vault_save(strategy="update")`

### Migration Guide
```
# Before (v0.5.0)
vault_create_note(title="...", content="...")
vault_quick_capture(content="...")

# After (v0.6.0)
vault_save(title="...", content="...", strategy="create")
vault_save(title="memo", content="...", strategy="inbox")

# New: Smart save with duplicate detection
vault_save(title="...", content="...", gist="...", strategy="smart")
```

## [0.5.0] - 2026-01-08

### Fixed
- **Plugin**: RelatedNotesView click navigation not working
  - Changed from `openLinkText` to `getLeaf().openFile()` pattern
  - Fixed timing issue where view opened before index was restored
  - Added `refresh()` method to force update after sync

### Added
- **Plugin**: Debug mode setting for verbose logging
  - New `Logger` class with component-based logging
  - Toggle in settings to enable/disable debug output
- **MCP**: `related --semantic` option for HTP-based similarity search
  - Uses gist embeddings instead of tag overlap
  - `--limit` flag to control result count
  - `--json` output format

### Changed
- **MCP**: Simplified `SearchEngine` - removed ONNX model dependency
  - HTP (Harmonic Token Projection) is now the only embedding method
  - No external model download required
  - Consistent with plugin's WASM implementation

## [0.4.0] - 2026-01-07

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
