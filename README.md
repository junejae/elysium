# Elysium

MCP server for Obsidian-based Second Brain with AI-powered semantic search.

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

## Usage

### As MCP Server

```bash
# Run MCP server (default behavior)
elysium-mcp

# Or directly with binary
elysium
```

### Claude Desktop Configuration

Add to `~/.config/claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "elysium": {
      "command": "npx",
      "args": ["elysium-mcp"],
      "cwd": "/path/to/your/vault"
    }
  }
}
```

### CLI Commands

```bash
elysium validate      # Schema + wikilink validation
elysium audit         # Comprehensive audit
elysium health        # Health score (0-100)
elysium status        # Vault snapshot
elysium search "q"    # Full-text search
elysium ss "query"    # Semantic search
elysium index         # Build search index
```

## MCP Tools

| Tool | Description |
|------|-------------|
| `vault_search` | Semantic search using gist embeddings |
| `vault_get_note` | Get note content and metadata |
| `vault_list_notes` | List notes with type/area filters |
| `vault_health` | Get vault health score (0-100) |
| `vault_status` | Get note counts by type/area |
| `vault_audit` | Run policy compliance audit |

## Project Structure

```
elysium/
├── mcp/        # MCP server (Rust)
├── npm/        # npm wrapper package
└── plugin/     # Obsidian plugin (coming soon)
```

## Technical Details

- **Embeddings**: HTP (Harmonic Token Projection) - local, no API required
- **Storage**: SQLite for vector storage and full-text search
- **Protocol**: MCP over stdio

## License

MIT
