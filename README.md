# Elysium

MCP server for Obsidian-based Second Brain with AI-powered semantic search.

## Features

- **Semantic Search**: Find notes by meaning using gist field embeddings
- **Schema Validation**: Enforce consistent YAML frontmatter across notes
- **Wikilink Integrity**: Detect broken internal links
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
cargo build --release
```

## Quick Start

1. **Set vault path**:
```bash
export ELYSIUM_VAULT_PATH="/path/to/your/vault"
```

2. **Configure Claude** (see MCP Configuration below)

3. **Use MCP tools**: `vault_search`, `vault_status`, `vault_audit`, etc.

## Configuration

### Environment Variable

Set `ELYSIUM_VAULT_PATH` to your Obsidian vault root:

```bash
export ELYSIUM_VAULT_PATH="/path/to/your/vault"
```

If not set, elysium uses the current working directory.

### Config File (.elysium.json)

Create `.elysium.json` in your vault root:

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

## MCP Server

### Starting the Server

```bash
# Via npm
npx elysium-mcp

# Or run binary directly
./elysium
```

### MCP Tools

| Tool | Description |
|------|-------------|
| `vault_search` | Semantic search using gist embeddings |
| `vault_related` | Find related notes with type/area boosting |
| `vault_get_note` | Get note content and metadata |
| `vault_list_notes` | List notes with type/area filters |
| `vault_status` | Get note counts by type/area |
| `vault_audit` | Run policy compliance audit |
| `vault_get_inbox` | Get inbox content with processing guide |
| `vault_clear_inbox` | Clear inbox after processing |
| `vault_save` | **Unified save interface** (see below) |

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
- **Index Export**: Exports index for MCP server to read

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
- **Storage**: SQLite for vector storage
- **Protocol**: MCP over stdio
- **Language**: Rust with async tokio runtime

## Project Structure

```
elysium/
├── mcp/                # MCP server (Rust)
│   ├── src/
│   │   ├── core/       # Note, schema, config modules
│   │   ├── mcp/        # MCP server implementation
│   │   ├── search/     # Embedding and vector search
│   │   └── tags/       # Tag management
│   └── Cargo.toml
├── plugin/             # Obsidian plugin (TypeScript + WASM)
├── npm/                # npm wrapper package
└── .github/workflows/  # CI/CD
```

## Development

```bash
# MCP Server
cd mcp
cargo build
cargo test

# Plugin
cd plugin
npm install
npm run build
npm run dev  # Watch mode
```

## Refactor Readiness

- Safety net checklist: `docs/refactor-safety-net.md`
- Fixtures: `tests/fixtures/vault_small`
- Contracts: `docs/contracts/` (plugin index contract v1)
- Fixture content (for golden baselines): alpha (tags: alpha, demo; gist keywords: work note), beta (tag: beta; gist keywords: tech term), gamma (area: learning; gist keywords: learning project)

Smoke tests:
```bash
# MCP smoke tests (uses fixtures)
cd mcp
cargo test

# Plugin index export validation (run from your vault root)
cd plugin
npm run smoke:index-export -- --vault /path/to/your/vault
```

## License

MIT
