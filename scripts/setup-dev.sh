#!/bin/bash
# Development environment setup script
# Run this once after cloning the repository

set -e

echo "Setting up Elysium development environment..."

# Install pre-commit hook
if [ -f .git/hooks/pre-commit ]; then
    echo "Pre-commit hook already exists, backing up..."
    mv .git/hooks/pre-commit .git/hooks/pre-commit.backup
fi

cp scripts/pre-commit .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit
echo "✅ Pre-commit hook installed"

# Check Rust toolchain
if command -v rustc &> /dev/null; then
    echo "✅ Rust installed: $(rustc --version)"
else
    echo "❌ Rust not found. Install from https://rustup.rs/"
    exit 1
fi

# Check cargo fmt
if command -v rustfmt &> /dev/null; then
    echo "✅ rustfmt installed"
else
    echo "Installing rustfmt..."
    rustup component add rustfmt
fi

# Check cargo clippy
if command -v cargo-clippy &> /dev/null; then
    echo "✅ clippy installed"
else
    echo "Installing clippy..."
    rustup component add clippy
fi

echo ""
echo "🎉 Development environment ready!"
echo ""
echo "Quick commands:"
echo "  cd mcp && cargo build       # Build"
echo "  cd mcp && cargo test        # Test"
echo "  cd mcp && cargo fmt         # Format code"
echo "  cd mcp && cargo clippy      # Lint"
