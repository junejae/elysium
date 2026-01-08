# Contributing to Elysium

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
