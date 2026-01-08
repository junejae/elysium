# Elysium

MCP server for Obsidian-based Second Brain with AI-powered semantic search.

## Features

- **Semantic Search**: Find notes by meaning using gist field embeddings
- **Schema Validation**: Enforce consistent YAML frontmatter across notes
- **Wikilink Integrity**: Detect and fix broken internal links
- **Vault Health**: Score and track note quality over time
- **Folder-agnostic**: Your folder structure, your choice - Elysium scans recursively

## Installation

### npm (Recommended)

```bash
npm install -g elysium-mcp
```

### From Source

```bash
git clone https://github.com/junejae/elysium.git
cd elysium/mcp
cargo build --release --features mcp
```

## Quick Start

1. **Set vault path** (required):
```bash
export ELYSIUM_VAULT_PATH="/path/to/your/vault"
```

2. **Generate config file** (optional):
```bash
elysium init --config
```

3. **Build search index**:
```bash
elysium index
```

## Configuration

### Environment Variable

Set `ELYSIUM_VAULT_PATH` to your Obsidian vault root:

```bash
export ELYSIUM_VAULT_PATH="/path/to/your/vault"
```

If not set, elysium uses the current working directory.

### Config File (.elysium.json)

Generate a config file in your vault root:

```bash
elysium init --config
```

This creates `.elysium.json` with customizable settings:

```json
{
  "version": 1,
  "schema": {
    "types": ["note", "term", "project", "log"],
    "statuses": ["active", "done", "archived"],
    "areas": ["work", "tech", "life", "career", "learning", "reference"],
    "required_fields": ["type", "status", "area", "gist"],
    "max_tags": 5,
    "lowercase_tags": true,
    "allow_hierarchical_tags": false
  },
  "features": {
    "inbox": "inbox.md",
    "wikilinks": true
  }
}
```

#### Schema Configuration

| Field | Default | Description |
|-------|---------|-------------|
| `types` | `["note", "term", "project", "log"]` | Valid note type values |
| `statuses` | `["active", "done", "archived"]` | Valid status values |
| `areas` | `["work", "tech", ...]` | Valid area values |
| `required_fields` | `["type", "status", "area", "gist"]` | Required frontmatter fields |
| `max_tags` | `5` | Maximum tags per note |
| `lowercase_tags` | `true` | Require lowercase tags |
| `allow_hierarchical_tags` | `false` | Allow tags with `/` |

#### Feature Configuration

| Field | Default | Description |
|-------|---------|-------------|
| `inbox` | `inbox.md` | Quick capture file path |
| `wikilinks` | `true` | Enable wikilink validation |

## CLI Commands

### Core Commands

```bash
# Initialize config
elysium init              # Check config
elysium init --config     # Generate .elysium.json

# Validation
elysium validate          # Full validation (schema + wikilinks)
elysium validate --schema # Schema only
elysium validate --wikilinks # Wikilinks only

# Audit
elysium audit             # Full audit
elysium audit --quick     # Quick audit (schema + wikilinks)
elysium audit --strict    # Exit 1 on violations

# Status
elysium status            # Vault summary
elysium status --brief    # Brief output
elysium health            # Health score (0-100)
elysium health --details  # Detailed breakdown
```

### Search Commands

```bash
# Full-text search
elysium search "query"
elysium search "query" --gist  # Search gist field only

# Semantic search
elysium ss "query"             # Alias for semantic-search
elysium semantic-search "query"
elysium ss "query" --limit 10

# Index management
elysium index             # Build/update index
elysium index --status    # Check index status
elysium index --rebuild   # Force rebuild
```

### Utility Commands

```bash
elysium tags              # List all tags
elysium tags --analyze    # Tag analysis and suggestions

elysium related "note"    # Find related notes

elysium fix --wikilinks   # Fix broken wikilinks (dry-run)
elysium fix --execute     # Apply fixes
```

## MCP Server

### Starting the Server

```bash
# Default: starts MCP server
elysium

# Or via npm
npx elysium-mcp

# Show installation instructions
elysium mcp --install
```

### MCP Tools

| Tool | Description |
|------|-------------|
| `vault_search` | Semantic search using gist embeddings |
| `vault_get_note` | Get note content and metadata |
| `vault_list_notes` | List notes with type/area filters |
| `vault_health` | Get vault health score (0-100) |
| `vault_status` | Get note counts by type/area |
| `vault_audit` | Run policy compliance audit |
| `vault_get_inbox` | Get inbox content with processing guide |
| `vault_clear_inbox` | Clear inbox after processing |
| `vault_save` | **Unified save interface** (see below) |
| `vault_get_stale_gists` | Find notes with outdated gists |

#### vault_save Strategies

| Strategy | Description |
|----------|-------------|
| `create` | Create new note with frontmatter |
| `update` | Overwrite existing note |
| `append` | Add content to existing note |
| `inbox` | Quick capture to inbox.md |
| `smart` | Auto-detect duplicates, suggest action |

Example:
```json
{
  "title": "GPU MIG",
  "content": "Multi-Instance GPU...",
  "note_type": "term",
  "area": "tech",
  "gist": "GPU MIG allows...",
  "source": "https://docs.nvidia.com/...",
  "strategy": "smart"
}
```

### Claude Desktop Configuration

Add to `~/.config/claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "elysium": {
      "command": "npx",
      "args": ["elysium-mcp"],
      "env": {
        "ELYSIUM_VAULT_PATH": "/path/to/your/vault"
      }
    }
  }
}
```

### Claude Code Configuration

Add to `.mcp.json` in your vault root:

```json
{
  "mcpServers": {
    "elysium": {
      "command": "npx",
      "args": ["elysium-mcp"],
      "env": {
        "ELYSIUM_VAULT_PATH": "/path/to/your/vault"
      }
    }
  }
}
```

## Vault Structure

Elysium is **folder-agnostic** - organize your vault however you like. 
Notes are scanned recursively from vault root (excluding dot-folders like `.obsidian`).

Example structure (your choice):

```
vault/
├── Notes/              # Optional - organize however you like
├── Projects/           # Optional
├── Archive/            # Optional
├── _system/            # Optional - dashboards, templates
├── inbox.md            # Quick capture (configurable path)
└── .elysium.json       # Config (optional)
```

## YAML Frontmatter Schema

Every note should have a YAML frontmatter block:

```yaml
---
type: note | term | project | log
status: active | done | archived
area: work | tech | life | career | learning | reference
gist: >
  2-3 sentence summary describing the note content
  for semantic search. Max 100 words.
tags: [lowercase, flat, max-five]
---
```

## Obsidian Plugin

The Elysium plugin adds vault management features to Obsidian:

### Features

- **Status Bar**: Shows vault health score
- **Quick Capture**: `Cmd+Shift+N` - quickly add memo to inbox
- **Semantic Search**: Search by meaning, not just keywords
- **Tag Cloud**: Visual tag browser

### Installation

1. Build plugin:
```bash
cd plugin && npm run build
```

2. Copy to vault:
```bash
cp main.js manifest.json styles.css /path/to/vault/.obsidian/plugins/elysium/
```

3. Enable in Obsidian: Settings → Community plugins → Elysium

## Technical Details

- **Embeddings**: HTP (Harmonic Token Projection) - local, no API required
- **Storage**: SQLite for vector storage and full-text search
- **Protocol**: MCP over stdio
- **Language**: Rust with async tokio runtime

## Project Structure

```
elysium/
├── mcp/                # MCP server (Rust)
│   ├── src/
│   │   ├── core/       # Note, schema, config modules
│   │   ├── commands/   # CLI commands
│   │   ├── mcp/        # MCP server implementation
│   │   └── search/     # Embedding and vector search
│   └── Cargo.toml
├── plugin/             # Obsidian plugin (TypeScript)
├── npm/                # npm wrapper package
└── .github/workflows/  # CI/CD
```

## Development

```bash
# MCP Server
cd mcp
cargo build --features mcp
cargo test
cargo fmt && cargo clippy

# Plugin
cd plugin
npm install
npm run build
npm run dev  # Watch mode
```

## License

MIT
