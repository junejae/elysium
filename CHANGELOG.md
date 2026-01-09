# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.4.0] - 2026-01-09

### Added
- **Model2Vec Integration**: Optional neural embedding model for improved semantic search
  - `embedder.rs`: Embedder trait abstraction over HTP and Model2Vec
  - `ModelDownloader`: Download Model2Vec (potion-base-8M, ~8MB) from HuggingFace
  - SetupWizard: "Advanced Semantic Search" step for optional model download
  - `AdvancedSemanticSearchConfig`: Configuration for neural embeddings

### Changed
- Search engine now supports swappable embedding backends (HTP default, Model2Vec optional)
- VectorDB updated to handle variable embedding dimensions

## [1.3.0] - 2026-01-09

### Added
- **Tag Automation**: Semantic tag generation using Model2Vec embeddings
  - `TagEmbedder`: Model2Vec wrapper (potion-multilingual-128M, 256 dims, 101 languages)
  - `TagDatabase`: SQLite-based tag storage with embeddings and aliases
  - `TagMatcher`: Hybrid matching combining keyword and semantic similarity
  - `KeywordExtractor`: Extract representative keywords from content using tokenizer
  - Tag Discovery mode: Find new tags from content, not just pre-indexed database

- **CLI Commands**:
  - `elysium tags init`: Initialize tag database with 31 seed tags
  - `elysium tags suggest`: Suggest tags for text (`--discover` for new tag discovery)
  - `elysium tags sync`: Sync all notes with auto-generated tags (`--discover --execute`)
  - `elysium tags extract`: Build tag DB from existing vault notes
  - `elysium tags keywords`: Extract keywords from text using Model2Vec tokenizer

- **MCP Integration**:
  - `vault_save`: Added `auto_tag`, `tag_limit`, `discover` parameters
  - Auto-tagging on note creation with discovery mode support

- **Stopword Filtering**:
  - 160+ English stopwords (articles, pronouns, prepositions, conjunctions)
  - 100+ Korean stopwords (조사, 대명사, 접속사, 부사)
  - Korean verb suffix patterns (~입니다, ~습니다, ~하는)
  - Korean particle patterns (~가, ~는, ~를)

### Dependencies
- Added `model2vec = "0.2"` for semantic embeddings
- Added `safetensors = "0.4"` for direct model access
- Added `tokenizers = "0.20"` for keyword extraction

## [1.2.0] - 2026-01-09

### Added
- **Release**: Unified version management across all components
  - `sync_versions()` function in `release.sh` syncs VERSION to all component files
  - Single source of truth: `/VERSION` file
  - Components synced: `plugin/manifest.json`, `plugin/package.json`, `mcp/Cargo.toml`, `npm/package.json`
  - Version sync runs automatically before git tag creation

### Changed
- **Plugin**: Version aligned to match MCP/npm (0.9.0 → 1.2.0)
  - All components now share the same version number
  - Simplifies release tracking and compatibility

## [1.1.0] - 2026-01-09

### Removed
- **BREAKING**: Algorithm-based gist auto-generation removed
  - `autoGenerate` and `trackSource` options removed from `GistConfig`
  - `generateGistFromContent()` function removed from `MigrationEngine`
  - `elysium_gist_source` and `elysium_gist_date` fields no longer used
  - Gist is now filled by AI (via MCP) or human, avoiding YAML corruption issues

### Changed
- **Audit**: Gist check now returns "warn" instead of "fail" for empty gist
  - Empty gist is not critical - can be filled later
  - Shows list of files with missing gist (first 5)
- **Plugin Settings**: Simplified gist options
  - Only "Enable Gist" and "Max length" remain
  - Removed auto-generate and track source toggles

### Fixed
- **YAML Corruption**: Fixed issue where algorithm-based gist extraction could
  corrupt YAML frontmatter by including code blocks, markdown images, or other
  content in the gist field

## [1.0.1] - 2026-01-09

### Fixed
- **MCP**: `vault_related` tool was not registered in MCP server
  - Added missing doc comment required by rmcp `tool_router` macro
  - Tool now properly appears in `tools/list` response

## [1.0.0] - 2026-01-09

### Changed
- **BREAKING**: Schema field names are now fixed and not user-configurable
  - All fields use `elysium_` prefix: `elysium_type`, `elysium_status`, `elysium_area`, `elysium_gist`, `elysium_tags`
  - Removed field name customization from plugin settings (values still configurable)
  - Config version upgraded to v3 with auto-migration from v1/v2
- **Plugin**: Simplified schema configuration
  - Replaced `filterableFields` structure with flat `typeValues`, `statusValues`, `areaValues`
  - Added `FIELD_NAMES` constant for consistent field name access

### Migration
Existing configs (v1/v2) are automatically migrated to v3 format.
Field values are preserved; only field names change to standardized `elysium_*` format.

## [0.9.0] - 2026-01-09

### Added
- **Plugin**: Folders configuration in Obsidian settings (notes, projects, archive)

### Changed
- **Architecture**: Plugin config is now Single Source of Truth (SSOT)
  - MCP reads from `.obsidian/plugins/elysium/config.json` first
  - Falls back to `.elysium.json` for backward compatibility
- **Philosophy**: MCP is a helper tool for the plugin, not standalone
  - Documented in CONTRIBUTING.md

## [0.8.0] - 2026-01-09

### Added
- **CLI/MCP**: `--boost-type` and `--boost-area` flags for related notes search
  - Re-ranks search results by boosting notes with same type/area as source note
  - Algorithm: `final_score = 0.7 × semantic_score + 0.3 × metadata_score`
  - Metadata score: +0.5 for same type, +0.5 for same area
- **MCP**: `vault_related` tool with `boost_type` and `boost_area` parameters
- **Config**: `folders` section in `.elysium.json` for configurable folder paths
  - `notes`: Folder for note/term/log types (default: "Notes")
  - `projects`: Folder for active projects (default: "Projects")
  - `archive`: Folder for archived projects (default: "Archive")

### Changed
- **MCP**: `vault_save` now uses configurable folder paths instead of hardcoded values
- **CLI**: `elysium init` now displays configured folder paths

## [0.7.2] - 2026-01-08

### Changed
- **BREAKING**: `vault_health` removed - merged into `vault_status`
  - `vault_status` now returns `health` object with score and coverage metrics
- **BREAKING**: `vault_get_stale_gists` removed - merged into `vault_audit`
  - `vault_audit` now includes `stale_gists` check (in full mode, not quick)

### Migration Guide
```
# Before (v0.6.x)
vault_status()   → {total_notes, by_type, by_area}
vault_health()   → {score, gist_coverage, type_coverage, area_coverage}
vault_get_stale_gists() → {count, notes: [...]}

# After (v0.7.2)
vault_status()   → {total_notes, by_type, by_area, health: {score, ...}}
vault_audit()    → {checks: [..., {id: "stale_gists", ...}]}
```

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
