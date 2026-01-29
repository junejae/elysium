# Refactor Safety Net

This checklist and test plan defines the minimum guardrails to refactor Elysium safely across MCP, plugin, wasm, and npm boundaries.

## Goals
- Prevent silent breakage across MCP ↔ plugin ↔ wasm contracts.
- Catch regressions in indexing/search correctness, ranking, and stability.
- Keep the smallest possible test surface to unblock refactors.

## Minimum Checklist (Gate)
- [ ] **Contract versioning**: MCP request/response schema versioned and validated at runtime.
- [ ] **Contract docs**: plugin index schema captured in `docs/contracts/`.
- [ ] **Path safety**: normalize paths, block traversal (`..`), enforce vault root, define symlink policy.
- [ ] **Smoke tests**: MCP boot + index tiny vault + query roundtrip.
- [ ] **Golden ranking**: stable query results on a fixed small vault.
- [ ] **Plugin lifecycle**: wasm → IndexedDB → watcher → UI order enforced (state machine or guard).
- [ ] **Docs alignment**: README/config/frontmatter docs match code expectations.

## Test Plan (Minimum Viable Suite)

### 1) MCP Smoke Test (Rust)
Purpose: ensure server starts and core tools respond.

- Setup a tiny vault fixture (3–5 notes with frontmatter + wikilinks).
- Start MCP server with `ELYSIUM_VAULT_PATH` pointing to fixture.
- Assert MCP tool calls succeed:
  - `vault_status`
  - `vault_search` (simple query)
  - `vault_get_note` (known note)
  - `vault_audit`

Expected outputs should be deterministic for the fixture.

### 2) Indexing & Search Roundtrip (Plugin/Indexer)
Purpose: prevent regressions in indexing pipeline.

- Run Indexer against fixture vault (or mocked Obsidian App).
- Assert:
  - Indexed doc count equals fixture file count.
  - Embedding dimension is stable.
  - Query returns deterministic top‑K IDs.

### 3) Ranking Baseline (Golden Results)
Purpose: protect relevance changes during refactor.

- Store golden results for 3–5 representative queries.
- Baseline file: `tests/fixtures/golden/search_baselines.json` (supports multiple modes).
- Use a fixed seed and deterministic embeddings (if possible).
- Tolerance policy: expected result must appear within top `maxRank` (default 1).

### 4) Data/Storage Migration Check
Purpose: prevent IndexedDB corruption or forced reindex.

- Version the storage schema.
- Test that a previous version can:
  - Load cleanly, or
  - Trigger a safe rebuild path.

### 5) Security Regression Checks
Purpose: prevent vault escape and unsafe writes.

- Path traversal: titles with `..`, `/`, `\` must be rejected or sanitized.
- Symlink policy: ensure reads/writes stay inside vault root.
- Input bounds: large note content/query should fail fast with clear errors.

## Fixture Guidelines
- `tests/fixtures/vault_small/`
- Include:
  - A note with `elysium_*` frontmatter
  - A note with wikilinks
  - A note with tags (max + invalid)
  - A malformed note to exercise audit
- Baseline fixtures used by golden search:
  - `alpha.md`: tags `alpha`, `demo`; gist keywords `work note`
  - `beta.md`: tag `beta`; gist keywords `tech term`
  - `gamma.md`: area `learning`; gist keywords `learning project`

## Suggested Tooling (Future)
- `cargo test -p elysium-mcp` for MCP/unit tests
- `npm test` (plugin) with a mocked Obsidian app
- Optional: `wasm-pack test` for wasm unit tests

## Refactor Readiness Exit Criteria
- All smoke tests pass locally.
- Contract version updated with changelog entry.
- Golden ranking diffs reviewed and accepted.
- Docs updated for any schema/config change.
