#!/bin/bash
# Release script for Elysium
# Usage: ./scripts/release.sh [--dry-run]
#
# This script:
# 1. Reads version from VERSION file
# 2. Validates CHANGELOG.md has entry for this version
# 3. Creates and pushes git tag
# 4. GitHub Actions handles the rest (build, release, npm publish)

set -e

DRY_RUN=false
if [[ "$1" == "--dry-run" ]]; then
    DRY_RUN=true
    echo "=== DRY RUN MODE ==="
fi

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Get script directory and project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
cd "$PROJECT_ROOT"

# Read version from VERSION file
if [[ ! -f VERSION ]]; then
    echo -e "${RED}ERROR: VERSION file not found${NC}"
    exit 1
fi

VERSION=$(cat VERSION | tr -d '[:space:]')
TAG="v$VERSION"

echo "=== Elysium Release Script ==="
echo "Version: $VERSION"
echo "Tag: $TAG"
echo ""

# Check if we're on main branch
CURRENT_BRANCH=$(git branch --show-current)
if [[ "$CURRENT_BRANCH" != "main" ]]; then
    echo -e "${YELLOW}WARNING: Not on main branch (current: $CURRENT_BRANCH)${NC}"
    read -p "Continue anyway? [y/N] " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

# Check for uncommitted changes
if [[ -n $(git status --porcelain) ]]; then
    echo -e "${RED}ERROR: Uncommitted changes detected${NC}"
    git status --short
    exit 1
fi

# Check if tag already exists
if git rev-parse "$TAG" >/dev/null 2>&1; then
    echo -e "${RED}ERROR: Tag $TAG already exists${NC}"
    echo "To delete and recreate: git tag -d $TAG && git push origin :refs/tags/$TAG"
    exit 1
fi

# Check CHANGELOG.md has entry for this version
if ! grep -q "## \[$VERSION\]" CHANGELOG.md; then
    echo -e "${RED}ERROR: CHANGELOG.md missing entry for version $VERSION${NC}"
    echo "Add a section like: ## [$VERSION] - $(date +%Y-%m-%d)"
    exit 1
fi

echo -e "${GREEN}Pre-flight checks passed!${NC}"
echo ""

# Show what will be released
echo "=== Release Notes (from CHANGELOG.md) ==="
# Extract the section for this version
sed -n "/## \[$VERSION\]/,/## \[/p" CHANGELOG.md | head -n -1
echo ""

if [[ "$DRY_RUN" == true ]]; then
    echo -e "${YELLOW}DRY RUN: Would create and push tag $TAG${NC}"
    exit 0
fi

# Confirm release
read -p "Create and push tag $TAG? [y/N] " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Aborted."
    exit 1
fi

# Create tag
echo "Creating tag $TAG..."
git tag -a "$TAG" -m "Release $VERSION"

# Push tag
echo "Pushing tag to origin..."
git push origin "$TAG"

echo ""
echo -e "${GREEN}=== Release $VERSION initiated! ===${NC}"
echo ""
echo "GitHub Actions will now:"
echo "  1. Build binaries for Linux, macOS, Windows"
echo "  2. Create GitHub Release with artifacts"
echo "  3. Publish to npm as elysium-mcp"
echo ""
echo "Monitor progress: https://github.com/junejae/elysium/actions"
