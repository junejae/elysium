# Contributing to Elysium

## Philosophy

**MCP is a helper tool for the Obsidian plugin, not a standalone product.**

| Principle | Description |
|-----------|-------------|
| **Plugin owns config** | All settings live in `.obsidian/plugins/elysium/config.json` |
| **MCP follows plugin** | MCP reads plugin config, never defines its own schema |
| **Single Source of Truth** | Plugin settings = the only truth. MCP adapts. |
| **Backward compatibility** | MCP falls back to `.elysium.json` for migration |

When adding features:
- If it needs config → add to Plugin first, MCP reads it
- If it's UI → Plugin only
- If it's CLI/automation → MCP, using Plugin's config

## Development Setup

```bash
# Clone and setup
git clone https://github.com/junejae/elysium.git
cd elysium
./scripts/setup-dev.sh
```

This installs pre-commit hooks for automatic formatting checks.

## Before Committing

```bash
cd mcp
cargo fmt      # Format code
cargo clippy   # Lint
cargo test     # Run tests
```

## Pre-commit Hook

The pre-commit hook runs `cargo fmt --check` automatically. If it fails:

```bash
cd mcp && cargo fmt
git add -A && git commit
```

## CI Checks

All PRs must pass:
- `cargo check` - Compilation
- `cargo test` - Tests
- `cargo clippy` - Lints
- `cargo fmt --check` - Formatting
