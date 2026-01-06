# Elysium

MCP server for Obsidian-based Second Brain with AI-powered semantic search.

## Features

- **Semantic Search**: Find notes by meaning using gist field embeddings
- **Schema Validation**: Enforce consistent YAML frontmatter across notes
- **Wikilink Integrity**: Detect and fix broken internal links
- **Vault Health**: Score and track note quality over time
- **Configurable**: Customize folder structure and schema rules per vault

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

2. **Generate config file** (optional but recommended):
```bash
elysium init --config
```

3. **Initialize folder structure**:
```bash
elysium init --create
```

4. **Build search index**:
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
  "folders": {
    "notes": "Notes",
    "projects": "Projects",
    "archive": "Archive",
    "system": "_system",
    "templates": "_system/Templates",
    "attachments": "_system/Attachments",
    "inbox": "inbox.md"
  },
  "schema": {
    "types": ["note", "term", "project", "log"],
    "statuses": ["active", "done", "archived"],
    "areas": ["work", "tech", "life", "career", "learning", "reference"],
    "max_tags": 5,
    "lowercase_tags": true,
    "allow_hierarchical_tags": false,
    "required_fields": ["type", "status", "area", "gist"]
  },
  "features": {
    "semantic_search": true,
    "wikilink_validation": true,
    "footer_markers": true
  }
}
```

#### Folder Configuration

| Field | Default | Description |
|-------|---------|-------------|
| `notes` | `Notes` | Folder for note, term, and log types |
| `projects` | `Projects` | Folder for active projects |
| `archive` | `Archive` | Folder for completed/archived projects |
| `system` | `_system` | Folder for system files |
| `templates` | `_system/Templates` | Folder for note templates |
| `attachments` | `_system/Attachments` | Folder for media files |
| `inbox` | `inbox.md` | Quick capture file path |

#### Schema Configuration

| Field | Default | Description |
|-------|---------|-------------|
| `types` | `["note", "term", "project", "log"]` | Valid note type values |
| `statuses` | `["active", "done", "archived"]` | Valid status values |
| `areas` | `["work", "tech", ...]` | Valid area values |
| `max_tags` | `5` | Maximum tags per note |
| `lowercase_tags` | `true` | Require lowercase tags |
| `allow_hierarchical_tags` | `false` | Allow tags with `/` |
| `required_fields` | `["type", "status", "area", "gist"]` | Required frontmatter fields |

## CLI Commands

### Core Commands

```bash
# Initialize vault structure
elysium init              # Check structure
elysium init --create     # Create missing folders
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
elysium fix --footer      # Fix missing footer markers
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

Default folder structure:

```
vault/
├── Notes/              # All notes (note, term, log types)
├── Projects/           # Active projects
├── Archive/            # Completed projects
├── _system/
│   ├── Dashboards/     # Dataview queries
│   ├── Templates/      # Note templates
│   └── Attachments/    # Media files
├── .opencode/          # AI agent configuration
├── inbox.md            # Quick capture
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
├── npm/                # npm wrapper package
└── .github/workflows/  # CI/CD
```

## Development

```bash
# Build debug
cd mcp
cargo build --features mcp

# Build release
cargo build --release --features mcp

# Run tests
cargo test

# Format code
cargo fmt

# Lint
cargo clippy
```

## License

MIT
