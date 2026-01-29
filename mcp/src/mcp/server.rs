//! Vault MCP Server implementation

use anyhow::Result;
use rmcp::{
    handler::server::{tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    tool, tool_router, ErrorData as McpError, ServerHandler, ServiceExt,
};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::core::note::{collect_all_notes, collect_note_names};
use crate::core::paths::VaultPaths;
use crate::core::schema::SchemaValidator;
use crate::search::engine::SearchEngine;
use crate::search::hybrid::{HybridSearchEngine, SearchMode};
use crate::search::PluginSearchEngine;
use crate::tags::keyword::KeywordExtractor;
use crate::tags::{TagDatabase, TagEmbedder, TagMatcher};

use super::audit;
use super::helpers::{build_note_json, resolve_fields};
use super::params::{
    AuditParams, GetNoteParams, ListNotesParams, RelatedParams, SaveParams, SearchParams,
    SuggestTagsParams, TagsAnalyzeParams, TagsSuggestParams,
};
use super::types::{AuditResultJson, SearchResultJson};

/// Vault MCP Service
#[derive(Clone)]
pub struct VaultService {
    vault_path: PathBuf,
    db_path: PathBuf,
    tool_router: ToolRouter<Self>,
}

impl VaultService {
    pub fn new(vault_path: PathBuf) -> Self {
        let config = crate::core::config::Config::load(&vault_path);
        let paths = config.resolve_paths(&vault_path);
        let db_path = paths.search_db.clone();

        // Check for legacy DB locations and warn if migration needed
        Self::check_legacy_db_migration(&vault_path, &paths);

        Self {
            vault_path,
            db_path,
            tool_router: Self::tool_router(),
        }
    }

    /// Check for legacy DB files and print migration guidance
    fn check_legacy_db_migration(vault_path: &Path, paths: &crate::core::config::ResolvedPaths) {
        let legacy_search_db = vault_path.join(".opencode/tools/data/search.db");
        let legacy_tag_db = vault_path.join(".claude/data/tags.db");

        let mut has_legacy = false;

        if legacy_search_db.exists() && !paths.search_db.exists() {
            eprintln!(
                "[Migration] Legacy search.db found at: {}",
                legacy_search_db.display()
            );
            has_legacy = true;
        }

        if legacy_tag_db.exists() && !paths.tag_db.exists() {
            eprintln!(
                "[Migration] Legacy tags.db found at: {}",
                legacy_tag_db.display()
            );
            has_legacy = true;
        }

        if has_legacy {
            eprintln!(
                "[Migration] New unified location: {}",
                paths.data_dir.display()
            );
            eprintln!("[Migration] To migrate, move DB files to the new location:");
            if legacy_search_db.exists() {
                eprintln!(
                    "  mv \"{}\" \"{}\"",
                    legacy_search_db.display(),
                    paths.search_db.display()
                );
            }
            if legacy_tag_db.exists() {
                eprintln!(
                    "  mv \"{}\" \"{}\"",
                    legacy_tag_db.display(),
                    paths.tag_db.display()
                );
            }
        }
    }

    /// Get plugin search engine (reads index exported by Obsidian plugin)
    fn get_plugin_engine(&self) -> Result<PluginSearchEngine, McpError> {
        PluginSearchEngine::load(&self.vault_path).map_err(|e| {
            McpError::internal_error(format!("Failed to load plugin index: {}", e), None)
        })
    }

    /// Get hybrid search engine (BM25 + Semantic)
    fn get_hybrid_engine(&self) -> Result<HybridSearchEngine, McpError> {
        HybridSearchEngine::new(&self.vault_path).map_err(|e| {
            McpError::internal_error(format!("Failed to load hybrid search engine: {}", e), None)
        })
    }

    /// Legacy: Get self-managed search engine (deprecated, use plugin index instead)
    #[allow(dead_code)]
    fn get_engine(&self) -> Result<SearchEngine, McpError> {
        use crate::search::SearchConfig;

        // Load config to check for advanced search settings
        let config = crate::core::config::Config::load(&self.vault_path);
        let search_config = SearchConfig {
            use_advanced: config.features.is_advanced_search_ready(),
            model_path: config.features.get_model_path().map(|p| {
                // If path is relative, resolve it from vault root
                if p.starts_with('.') {
                    self.vault_path.join(p).to_string_lossy().to_string()
                } else {
                    p.to_string()
                }
            }),
            model_id: Some(config.features.advanced_semantic_search.model_id.clone()),
        };

        SearchEngine::with_config(&self.vault_path, &self.db_path, search_config)
            .map_err(|e| McpError::internal_error(format!("Failed to create engine: {}", e), None))
    }

    fn get_vault_paths(&self) -> VaultPaths {
        VaultPaths::from_root(self.vault_path.clone())
    }

    fn get_schema_validator(&self) -> SchemaValidator {
        let vault_paths = self.get_vault_paths();
        SchemaValidator::from_config(&vault_paths.config.schema)
    }

    /// Get tag matcher for auto-tagging
    /// Returns None if tag DB is not initialized
    fn get_tag_matcher(&self) -> Option<TagMatcher> {
        let paths = self.get_resolved_paths();

        if !paths.tag_db.exists() {
            return None;
        }

        let embedder = TagEmbedder::default_multilingual().ok()?;
        let database = TagDatabase::open(&paths.tag_db).ok()?;

        Some(TagMatcher::new(embedder, database))
    }

    /// Get resolved paths helper
    fn get_resolved_paths(&self) -> crate::core::config::ResolvedPaths {
        let config = crate::core::config::Config::load(&self.vault_path);
        config.resolve_paths(&self.vault_path)
    }

    /// Suggest tags for given text using semantic matching
    fn suggest_tags(&self, text: &str, limit: usize, discover: bool) -> Vec<String> {
        let matcher = match self.get_tag_matcher() {
            Some(m) => m,
            None => return vec![],
        };

        // Load keyword extractor if discovery mode is enabled
        let keyword_extractor = if discover {
            KeywordExtractor::from_default_cache().ok()
        } else {
            None
        };

        matcher
            .suggest_tags_with_discovery(text, limit, keyword_extractor.as_ref())
            .ok()
            .map(|suggestions| suggestions.into_iter().map(|s| s.tag).collect())
            .unwrap_or_default()
    }
}

#[tool_router]
impl VaultService {
    /// Search notes using hybrid search (BM25 + semantic)
    #[tool(
        description = "Search Second Brain Vault using hybrid search (BM25 + semantic). Supports search modes: 'hybrid' (default), 'semantic' (HNSW only), 'keyword' (BM25 only). Returns notes with optional type/area filtering."
    )]
    async fn vault_search(
        &self,
        params: Parameters<SearchParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut engine = self.get_hybrid_engine()?;
        let note_type_filter = &params.0.note_type;
        let area_filter = &params.0.area;

        // Parse search mode (default: Hybrid)
        let search_mode = params
            .0
            .search_mode
            .as_deref()
            .map(SearchMode::from_str)
            .unwrap_or_default();

        // If filtering, fetch more results to account for filtered-out items
        let has_filter = note_type_filter.is_some() || area_filter.is_some();
        let fetch_multiplier = if has_filter { 5 } else { 1 };

        // Clamp limit: default 5, max 100 (DoS prevention)
        let limit = params.0.limit.max(1).min(100);
        let limit = if limit == 1 && params.0.limit == 0 {
            5
        } else {
            limit
        };

        let fetch_limit = (limit * fetch_multiplier).min(500);

        let results = engine
            .search(&params.0.query, fetch_limit, search_mode)
            .map_err(|e| McpError::internal_error(format!("Search failed: {}", e), None))?;

        // Build dynamic JSON based on fields parameter
        let (requested_fields, is_all) = resolve_fields(&params.0.fields);

        let json_results: Vec<HashMap<String, serde_json::Value>> = results
            .into_iter()
            .filter(|r| {
                // Apply note_type filter
                let type_match = note_type_filter
                    .as_ref()
                    .map_or(true, |t| r.note_type.as_ref().map_or(false, |nt| nt == t));
                // Apply area filter
                let area_match = area_filter
                    .as_ref()
                    .map_or(true, |a| r.area.as_ref().map_or(false, |na| na == a));
                type_match && area_match
            })
            .take(limit)
            .map(|r| {
                let mut result: HashMap<String, serde_json::Value> = HashMap::new();

                // Always include title, path, and score for search results
                result.insert("title".to_string(), serde_json::Value::String(r.title));
                result.insert("path".to_string(), serde_json::Value::String(r.path));
                result.insert("score".to_string(), serde_json::json!(r.score));

                // Include other fields based on request
                let include_field =
                    |field: &str| -> bool { is_all || requested_fields.contains(&field) };

                if include_field("gist") {
                    if let Some(gist) = r.gist {
                        result.insert("gist".to_string(), serde_json::Value::String(gist));
                    }
                }
                if include_field("type") {
                    if let Some(note_type) = r.note_type {
                        result.insert("type".to_string(), serde_json::Value::String(note_type));
                    }
                }
                if include_field("area") {
                    if let Some(area) = r.area {
                        result.insert("area".to_string(), serde_json::Value::String(area));
                    }
                }

                result
            })
            .collect();

        let output = serde_json::to_string_pretty(&json_results).map_err(|e| {
            McpError::internal_error(format!("JSON serialization failed: {}", e), None)
        })?;

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    /// Find related notes using semantic similarity
    #[tool(
        description = "Find related notes using semantic similarity with optional type/area boosting."
    )]
    async fn vault_related(
        &self,
        params: Parameters<RelatedParams>,
    ) -> Result<CallToolResult, McpError> {
        let vault_paths = self.get_vault_paths();
        let notes = collect_all_notes(&vault_paths);
        let note_name = &params.0.note;

        let found = notes.iter().find(|n| {
            n.name == *note_name
                || n.path.file_stem().map(|s| s.to_string_lossy().to_string())
                    == Some(note_name.clone())
        });

        let source_note = match found {
            Some(n) => n,
            None => {
                return Ok(CallToolResult::success(vec![Content::text(
                    serde_json::json!({"error": format!("Note '{}' not found", note_name)})
                        .to_string(),
                )]));
            }
        };

        let gist = match source_note.gist() {
            Some(g) if !g.is_empty() => g,
            _ => {
                return Ok(CallToolResult::success(vec![Content::text(
                    serde_json::json!({"error": "Note has no gist for semantic search"})
                        .to_string(),
                )]));
            }
        };

        let engine = self.get_plugin_engine()?;
        let limit = params.0.limit.max(1).min(50);

        // Note: boost_type and boost_area are currently ignored when using plugin index
        // TODO: Implement boost in PluginSearchEngine if needed
        let results = engine
            .search(gist, limit + 1)
            .map_err(|e| McpError::internal_error(format!("Search failed: {}", e), None))?;

        let filtered: Vec<SearchResultJson> = results
            .into_iter()
            .filter(|r| r.title != source_note.name)
            .take(limit)
            .map(|r| SearchResultJson {
                title: r.title,
                path: r.path,
                gist: r.gist,
                note_type: r.note_type,
                area: r.area,
                score: r.score,
            })
            .collect();

        let output = serde_json::to_string_pretty(&filtered).map_err(|e| {
            McpError::internal_error(format!("JSON serialization failed: {}", e), None)
        })?;

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(
        description = "Get the full content and metadata of a specific note from Second Brain Vault."
    )]
    async fn vault_get_note(
        &self,
        params: Parameters<GetNoteParams>,
    ) -> Result<CallToolResult, McpError> {
        let vault_paths = self.get_vault_paths();
        let notes = collect_all_notes(&vault_paths);
        let note_name = &params.0.note;

        // Find note by title or path
        let found = notes.into_iter().find(|n| {
            n.name == *note_name
                || n.path.to_string_lossy().contains(note_name)
                || n.path.file_stem().map(|s| s.to_string_lossy().to_string())
                    == Some(note_name.clone())
        });

        match found {
            Some(n) => {
                let content = std::fs::read_to_string(&n.path).map_err(|e| {
                    McpError::internal_error(format!("Failed to read note: {}", e), None)
                })?;

                // Build dynamic metadata based on fields parameter
                let metadata = build_note_json(&n, &params.0.fields);
                let metadata_json = serde_json::to_string_pretty(&metadata).unwrap_or_default();

                let output = format!(
                    "## Metadata\n```json\n{}\n```\n\n## Content\n{}",
                    metadata_json, content
                );

                Ok(CallToolResult::success(vec![Content::text(output)]))
            }
            None => Ok(CallToolResult::success(vec![Content::text(format!(
                "Note not found: {}",
                note_name
            ))])),
        }
    }

    /// List notes in the vault with optional filters
    #[tool(description = "List notes in Second Brain Vault with optional type/area filters.")]
    async fn vault_list_notes(
        &self,
        params: Parameters<ListNotesParams>,
    ) -> Result<CallToolResult, McpError> {
        let vault_paths = self.get_vault_paths();
        let notes = collect_all_notes(&vault_paths);
        let note_type = &params.0.note_type;
        let area = &params.0.area;
        // Clamp limit: default 50, max 500 (DoS prevention)
        let limit = params.0.limit.max(1).min(500);
        let limit = if limit == 1 && params.0.limit == 0 {
            50
        } else {
            limit
        };

        // Build dynamic JSON based on fields parameter
        let fields_param = &params.0.fields;
        let filtered: Vec<HashMap<String, serde_json::Value>> = notes
            .into_iter()
            .filter(|n| {
                note_type
                    .as_ref()
                    .map_or(true, |t| n.note_type().map_or(false, |nt| nt == t))
                    && area
                        .as_ref()
                        .map_or(true, |a| n.area().map_or(false, |na| na == a))
            })
            .take(limit)
            .map(|n| build_note_json(&n, fields_param))
            .collect();

        let output = serde_json::to_string_pretty(&filtered).map_err(|e| {
            McpError::internal_error(format!("JSON serialization failed: {}", e), None)
        })?;

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    /// Get vault status summary with health metrics
    #[tool(
        description = "Get Second Brain Vault status summary including note counts by type/area and health score (0-100)."
    )]
    async fn vault_status(&self) -> Result<CallToolResult, McpError> {
        let vault_paths = self.get_vault_paths();
        let notes = collect_all_notes(&vault_paths);

        let mut by_type: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut by_area: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        for note in &notes {
            if let Some(t) = note.note_type() {
                *by_type.entry(t.to_string()).or_insert(0) += 1;
            }
            if let Some(a) = note.area() {
                *by_area.entry(a.to_string()).or_insert(0) += 1;
            }
        }

        let total = notes.len();
        let with_gist = notes.iter().filter(|n| n.gist().is_some()).count();
        let with_type = notes.iter().filter(|n| n.note_type().is_some()).count();
        let with_area = notes.iter().filter(|n| n.area().is_some()).count();

        let gist_score = if total > 0 {
            (with_gist as f64 / total as f64) * 40.0
        } else {
            0.0
        };
        let type_score = if total > 0 {
            (with_type as f64 / total as f64) * 30.0
        } else {
            0.0
        };
        let area_score = if total > 0 {
            (with_area as f64 / total as f64) * 30.0
        } else {
            0.0
        };

        let health_score = (gist_score + type_score + area_score).round() as u32;

        let output = serde_json::json!({
            "total_notes": total,
            "by_type": by_type,
            "by_area": by_area,
            "health": {
                "score": health_score,
                "gist_coverage": format!("{:.0}%", if total > 0 { (with_gist as f64 / total as f64) * 100.0 } else { 0.0 }),
                "type_coverage": format!("{:.0}%", if total > 0 { (with_type as f64 / total as f64) * 100.0 } else { 0.0 }),
                "area_coverage": format!("{:.0}%", if total > 0 { (with_area as f64 / total as f64) * 100.0 } else { 0.0 }),
            }
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&output).unwrap_or_default(),
        )]))
    }

    /// Run vault policy compliance audit
    #[tool(
        description = "Run vault policy compliance audit. Returns check results for schema validation, wikilinks, folder-type matching, gist coverage, tag usage, and orphan detection."
    )]
    async fn vault_audit(
        &self,
        params: Parameters<AuditParams>,
    ) -> Result<CallToolResult, McpError> {
        let vault_paths = self.get_vault_paths();
        let notes = collect_all_notes(&vault_paths);
        let note_names = collect_note_names(&vault_paths);
        let quick = params.0.quick;
        let verbose = params.0.verbose;

        let validator = self.get_schema_validator();
        let schema_config = &vault_paths.config.schema;

        let mut checks = Vec::new();

        // Schema check
        let schema_check = audit::check_schema(&notes, &validator, schema_config, verbose);
        checks.push(schema_check);

        // Wikilinks check
        let wikilinks_check = audit::check_wikilinks(&notes, &note_names, verbose);
        checks.push(wikilinks_check);

        if !quick {
            // Gist coverage check
            let gist_check = audit::check_gist(&notes, verbose);
            checks.push(gist_check);

            // Tag usage check
            let tags_check = audit::check_tags(&notes, verbose);
            checks.push(tags_check);

            // Orphan notes check
            let orphans_check = audit::check_orphans(&notes, &note_names, verbose);
            checks.push(orphans_check);

            // Stale gists check
            let stale_gists_check = audit::check_stale_gists(&notes, verbose);
            checks.push(stale_gists_check);
        }

        let passed = checks.iter().filter(|c| c.status == "pass").count();
        let failed = checks.iter().filter(|c| c.status == "fail").count();

        let result = AuditResultJson {
            timestamp: chrono::Local::now().to_rfc3339(),
            total_checks: checks.len(),
            passed,
            failed,
            checks,
        };

        let output = serde_json::to_string_pretty(&result).map_err(|e| {
            McpError::internal_error(format!("JSON serialization failed: {}", e), None)
        })?;

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(
        description = "Get the content of the inbox file for AI processing. Returns content and processing instructions."
    )]
    async fn vault_get_inbox(&self) -> Result<CallToolResult, McpError> {
        let vault_paths = self.get_vault_paths();
        let inbox_path = vault_paths.config.resolve_paths(&self.vault_path).inbox;
        let schema = &vault_paths.config.schema;

        let processing_guide = serde_json::json!({
            "instructions": [
                "Parse each memo separated by '---'",
                "For each memo, determine: new note, append to existing, or discard",
                "Create notes at vault root using vault_create_note",
                "After all processing, call vault_clear_inbox"
            ],
                "schema": {
                "required_fields": ["elysium_type", "elysium_status", "elysium_area", "elysium_gist"],
                "type_values": schema.types,
                "status_values": schema.statuses,
                "area_values": schema.areas
            },
            "naming": {
                "log_notes": "YYYY-MM-DD title.md (for type=log)",
                "regular_notes": "Title.md"
            }
        });

        if !inbox_path.exists() {
            return Ok(CallToolResult::success(vec![Content::text(
                serde_json::json!({
                    "exists": false,
                    "path": inbox_path.to_string_lossy(),
                    "content": null,
                    "processing_guide": processing_guide
                })
                .to_string(),
            )]));
        }

        let content = std::fs::read_to_string(&inbox_path)
            .map_err(|e| McpError::internal_error(format!("Failed to read inbox: {}", e), None))?;

        let output = serde_json::json!({
            "exists": true,
            "path": inbox_path.to_string_lossy(),
            "content": content,
            "size": content.len(),
            "lines": content.lines().count(),
            "processing_guide": processing_guide
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&output).unwrap_or_default(),
        )]))
    }

    /// Clear inbox file after processing
    #[tool(
        description = "Clear the inbox file content after processing. Preserves the file but empties its content."
    )]
    async fn vault_clear_inbox(&self) -> Result<CallToolResult, McpError> {
        let vault_paths = self.get_vault_paths();
        let inbox_path = vault_paths.config.resolve_paths(&self.vault_path).inbox;

        if !inbox_path.exists() {
            return Ok(CallToolResult::success(vec![Content::text(
                "Inbox file does not exist",
            )]));
        }

        std::fs::write(&inbox_path, "")
            .map_err(|e| McpError::internal_error(format!("Failed to clear inbox: {}", e), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::json!({
                "success": true,
                "message": "Inbox cleared"
            })
            .to_string(),
        )]))
    }

    #[tool(
        description = "Unified save interface for vault notes. Supports strategies: 'create' (new note), 'update' (overwrite), 'append' (add content), 'inbox' (quick capture), 'smart' (auto-detect duplicates)."
    )]
    async fn vault_save(&self, params: Parameters<SaveParams>) -> Result<CallToolResult, McpError> {
        let strategy = params.0.strategy.to_lowercase();

        match strategy.as_str() {
            "create" => self.save_create(&params.0).await,
            "update" => self.save_update(&params.0).await,
            "append" => self.save_append(&params.0).await,
            "inbox" => self.save_inbox(&params.0).await,
            "smart" => self.save_smart(&params.0).await,
            _ => Ok(CallToolResult::success(vec![Content::text(
                serde_json::json!({
                    "success": false,
                    "error": format!("Unknown strategy: {}. Use: create, update, append, inbox, smart", strategy)
                })
                .to_string(),
            )])),
        }
    }
}

// Save strategy implementations
impl VaultService {
    fn get_target_folder(&self, _note_type: Option<&str>) -> PathBuf {
        let vault_paths = self.get_vault_paths();
        let folders = &vault_paths.config.folders;

        // Always use Notes/ folder for flat structure (vault policy)
        let folder = &folders.notes;
        let target = self.vault_path.join(folder);

        if !target.exists() {
            let _ = std::fs::create_dir_all(&target);
        }

        target
    }

    async fn save_create(&self, params: &SaveParams) -> Result<CallToolResult, McpError> {
        let filename = format!("{}.md", params.title);
        let target_folder = self.get_target_folder(params.note_type.as_deref());
        let note_path = target_folder.join(&filename);
        let root_path = self.vault_path.join(&filename);

        if note_path.exists() || root_path.exists() {
            let existing_path = if note_path.exists() {
                &note_path
            } else {
                &root_path
            };
            return Ok(CallToolResult::success(vec![Content::text(
                serde_json::json!({
                    "success": false,
                    "error": format!("Note already exists: {}", existing_path.to_string_lossy()),
                    "suggestion": "Use strategy='update' to overwrite or strategy='append' to add content"
                })
                .to_string(),
            )]));
        }

        let frontmatter = self.build_frontmatter(params);
        let full_content = format!("{}{}", frontmatter, params.content);

        std::fs::write(&note_path, &full_content)
            .map_err(|e| McpError::internal_error(format!("Failed to create note: {}", e), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::json!({
                "success": true,
                "action": "created",
                "path": note_path.to_string_lossy(),
                "folder": target_folder.file_name().unwrap_or_default().to_string_lossy(),
                "title": params.title
            })
            .to_string(),
        )]))
    }

    async fn save_update(&self, params: &SaveParams) -> Result<CallToolResult, McpError> {
        let vault_paths = self.get_vault_paths();
        let notes = collect_all_notes(&vault_paths);

        let found = notes.into_iter().find(|n| {
            n.name == params.title
                || n.path.file_stem().map(|s| s.to_string_lossy().to_string())
                    == Some(params.title.clone())
        });

        match found {
            Some(note) => {
                let frontmatter = self.build_frontmatter(params);
                let full_content = format!("{}{}", frontmatter, params.content);

                std::fs::write(&note.path, &full_content).map_err(|e| {
                    McpError::internal_error(format!("Failed to update note: {}", e), None)
                })?;

                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::json!({
                        "success": true,
                        "action": "updated",
                        "path": note.path.to_string_lossy(),
                        "title": params.title
                    })
                    .to_string(),
                )]))
            }
            None => Ok(CallToolResult::success(vec![Content::text(
                serde_json::json!({
                    "success": false,
                    "error": format!("Note not found: {}", params.title),
                    "suggestion": "Use strategy='create' to create a new note"
                })
                .to_string(),
            )])),
        }
    }

    async fn save_append(&self, params: &SaveParams) -> Result<CallToolResult, McpError> {
        let vault_paths = self.get_vault_paths();
        let notes = collect_all_notes(&vault_paths);

        let found = notes.into_iter().find(|n| {
            n.name == params.title
                || n.path.file_stem().map(|s| s.to_string_lossy().to_string())
                    == Some(params.title.clone())
        });

        match found {
            Some(note) => {
                let existing = std::fs::read_to_string(&note.path).map_err(|e| {
                    McpError::internal_error(format!("Failed to read note: {}", e), None)
                })?;

                let new_content = format!("{}\n\n{}", existing.trim_end(), params.content);

                std::fs::write(&note.path, &new_content).map_err(|e| {
                    McpError::internal_error(format!("Failed to append to note: {}", e), None)
                })?;

                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::json!({
                        "success": true,
                        "action": "appended",
                        "path": note.path.to_string_lossy(),
                        "title": params.title
                    })
                    .to_string(),
                )]))
            }
            None => Ok(CallToolResult::success(vec![Content::text(
                serde_json::json!({
                    "success": false,
                    "error": format!("Note not found: {}", params.title),
                    "suggestion": "Use strategy='create' to create a new note"
                })
                .to_string(),
            )])),
        }
    }

    async fn save_inbox(&self, params: &SaveParams) -> Result<CallToolResult, McpError> {
        let vault_paths = self.get_vault_paths();
        let inbox_path = vault_paths.config.resolve_paths(&self.vault_path).inbox;

        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
        let memo = if params.title.is_empty() || params.title == "inbox" {
            format!("\n---\n\n**{}**\n\n{}", timestamp, params.content)
        } else {
            format!(
                "\n---\n\n**{}** - {}\n\n{}",
                timestamp, params.title, params.content
            )
        };

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&inbox_path)
            .map_err(|e| McpError::internal_error(format!("Failed to open inbox: {}", e), None))?;

        use std::io::Write;
        file.write_all(memo.as_bytes()).map_err(|e| {
            McpError::internal_error(format!("Failed to write to inbox: {}", e), None)
        })?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::json!({
                "success": true,
                "action": "inbox_added",
                "path": inbox_path.to_string_lossy(),
                "timestamp": timestamp
            })
            .to_string(),
        )]))
    }

    async fn save_smart(&self, params: &SaveParams) -> Result<CallToolResult, McpError> {
        let threshold = params.similarity_threshold.unwrap_or(0.7);
        let search_query = params.gist.as_deref().unwrap_or(&params.title);

        let mut engine = self.get_engine()?;
        let similar = engine
            .search(search_query, 3)
            .map_err(|e| McpError::internal_error(format!("Search failed: {}", e), None))?;

        let high_similarity: Vec<_> = similar
            .into_iter()
            .filter(|r| r.score >= threshold)
            .collect();

        if high_similarity.is_empty() {
            return self.save_create(params).await;
        }

        let similar_notes: Vec<serde_json::Value> = high_similarity
            .iter()
            .map(|r| {
                serde_json::json!({
                    "title": r.title,
                    "path": r.path,
                    "similarity": format!("{:.0}%", r.score * 100.0),
                    "gist": r.gist
                })
            })
            .collect();

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::json!({
                "success": true,
                "action": "needs_decision",
                "similar_notes": similar_notes,
                "suggestion": format!(
                    "Found {} similar note(s). Options: strategy='create' to create anyway, strategy='append' with title='{}' to add to existing, or strategy='update' to overwrite.",
                    high_similarity.len(),
                    high_similarity[0].title
                )
            })
            .to_string(),
        )]))
    }

    /// Suggest tags for given text using semantic matching
    #[tool(
        description = "Suggest tags for text using semantic similarity. Uses Model2Vec embeddings to find relevant tags from the tag database."
    )]
    async fn vault_tags_suggest(
        &self,
        params: Parameters<TagsSuggestParams>,
    ) -> Result<CallToolResult, McpError> {
        let matcher = self.get_tag_matcher().ok_or_else(|| {
            McpError::internal_error(
                "Tag database not initialized. Run 'elysium tags init' first.".to_string(),
                None,
            )
        })?;

        let suggestions = matcher
            .suggest_tags_hybrid(&params.0.text, params.0.limit)
            .map_err(|e| {
                McpError::internal_error(format!("Failed to suggest tags: {}", e), None)
            })?;

        #[derive(Serialize)]
        struct TagSuggestionResult {
            tag: String,
            score: f32,
            reason: String,
        }

        let results: Vec<TagSuggestionResult> = suggestions
            .into_iter()
            .map(|s| TagSuggestionResult {
                tag: s.tag,
                score: s.score,
                reason: s.reason,
            })
            .collect();

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&serde_json::json!({
                "input": params.0.text,
                "suggestions": results,
                "count": results.len()
            }))
            .unwrap(),
        )]))
    }

    /// Analyze tags and suggest merges for similar tags
    #[tool(
        description = "Analyze tag database and suggest merges for similar tags. Returns pairs of tags that could be merged based on semantic similarity."
    )]
    async fn vault_tags_analyze(
        &self,
        params: Parameters<TagsAnalyzeParams>,
    ) -> Result<CallToolResult, McpError> {
        let matcher = self.get_tag_matcher().ok_or_else(|| {
            McpError::internal_error(
                "Tag database not initialized. Run 'elysium tags init' first.".to_string(),
                None,
            )
        })?;

        let merge_suggestions = matcher
            .analyze_for_merges(params.0.threshold)
            .map_err(|e| {
                McpError::internal_error(format!("Failed to analyze tags: {}", e), None)
            })?;

        #[derive(Serialize)]
        struct MergeResult {
            keep: String,
            merge: String,
            similarity: f32,
        }

        let results: Vec<MergeResult> = merge_suggestions
            .into_iter()
            .map(|s| MergeResult {
                keep: s.keep,
                merge: s.merge,
                similarity: s.similarity,
            })
            .collect();

        // Get tag stats
        let db = matcher.database();
        let total_tags = db.tag_count().unwrap_or(0);

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&serde_json::json!({
                "total_tags": total_tags,
                "threshold": params.0.threshold,
                "merge_suggestions": results,
                "suggestion_count": results.len()
            }))
            .unwrap(),
        )]))
    }

    /// List all tags in the database
    #[tool(
        description = "List all tags in the tag database with their descriptions and usage counts."
    )]
    async fn vault_tags_list(&self) -> Result<CallToolResult, McpError> {
        let paths = self.get_resolved_paths();

        if !paths.tag_db.exists() {
            return Err(McpError::internal_error(
                "Tag database not initialized. Run 'elysium tags init' first.".to_string(),
                None,
            ));
        }

        let db = TagDatabase::open(&paths.tag_db)
            .map_err(|e| McpError::internal_error(format!("Failed to open tag DB: {}", e), None))?;

        let tags = db
            .get_all_tags()
            .map_err(|e| McpError::internal_error(format!("Failed to get tags: {}", e), None))?;

        #[derive(Serialize)]
        struct TagInfo {
            name: String,
            description: String,
            aliases: Vec<String>,
            usage_count: i64,
        }

        let tag_list: Vec<TagInfo> = tags
            .into_iter()
            .map(|t| TagInfo {
                name: t.name,
                description: t.description,
                aliases: t.aliases,
                usage_count: t.usage_count,
            })
            .collect();

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&serde_json::json!({
                "total": tag_list.len(),
                "tags": tag_list
            }))
            .unwrap(),
        )]))
    }

    /// Suggest tags based on similar notes (Advanced Search feature)
    #[tool(
        description = "Suggest tags by finding similar notes using Advanced Semantic Search. Requires Advanced Search to be enabled. Aggregates tags from semantically similar notes."
    )]
    async fn vault_suggest_tags(
        &self,
        params: Parameters<SuggestTagsParams>,
    ) -> Result<CallToolResult, McpError> {
        // Check if advanced search is enabled
        let config = crate::core::config::Config::load(&self.vault_path);
        if !config.features.is_advanced_search_ready() {
            return Ok(CallToolResult::success(vec![Content::text(
                serde_json::json!({
                    "success": false,
                    "error": "Advanced Semantic Search is not enabled",
                    "suggestion": "Enable Advanced Semantic Search in plugin settings and download the model"
                })
                .to_string(),
            )]));
        }

        // Find the source note
        let vault_paths = self.get_vault_paths();
        let notes = collect_all_notes(&vault_paths);
        let note_name = &params.0.note;

        let source_note = notes.iter().find(|n| {
            n.name == *note_name
                || n.path.file_stem().map(|s| s.to_string_lossy().to_string())
                    == Some(note_name.clone())
                || n.path.to_string_lossy().contains(note_name)
        });

        let source_note = match source_note {
            Some(n) => n,
            None => {
                return Ok(CallToolResult::success(vec![Content::text(
                    serde_json::json!({
                        "success": false,
                        "error": format!("Note '{}' not found", note_name)
                    })
                    .to_string(),
                )]));
            }
        };

        // Check if note has a gist
        let gist = match source_note.gist() {
            Some(g) if !g.is_empty() => g,
            _ => {
                return Ok(CallToolResult::success(vec![Content::text(
                    serde_json::json!({
                        "success": false,
                        "error": "Note has no gist for semantic search",
                        "suggestion": "Add a gist to the note's frontmatter first"
                    })
                    .to_string(),
                )]));
            }
        };

        // Get the source note's existing tags to exclude them
        let source_tags: HashSet<String> = source_note.tags().into_iter().collect();

        // Search for similar notes
        let mut engine = self.get_engine()?;
        let similar_count = params.0.similar_count.max(1).min(50);

        let similar_notes = engine
            .search(gist, similar_count + 1) // +1 to account for self
            .map_err(|e| McpError::internal_error(format!("Search failed: {}", e), None))?;

        // Aggregate tags from similar notes
        let mut tag_counts: std::collections::HashMap<String, (usize, f32)> =
            std::collections::HashMap::new();

        for result in similar_notes.iter() {
            // Skip the source note itself
            if result.title == source_note.name {
                continue;
            }

            // Find the note to get its tags
            if let Some(note) = notes.iter().find(|n| n.name == result.title) {
                for tag in note.tags() {
                    // Skip tags the source note already has
                    if source_tags.contains(&tag) {
                        continue;
                    }

                    let entry = tag_counts.entry(tag).or_insert((0, 0.0));
                    entry.0 += 1;
                    entry.1 = entry.1.max(result.score); // Keep highest score
                }
            }
        }

        // Filter by minimum frequency and sort
        let min_freq = params.0.min_frequency.max(1);
        let mut suggestions: Vec<(String, usize, f32)> = tag_counts
            .into_iter()
            .filter(|(_, (count, _))| *count >= min_freq)
            .map(|(tag, (count, score))| (tag, count, score))
            .collect();

        // Sort by frequency (descending), then by score (descending)
        suggestions.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal))
        });

        suggestions.truncate(params.0.limit);

        #[derive(Serialize)]
        struct TagSuggestion {
            tag: String,
            frequency: usize,
            max_similarity: f32,
        }

        let results: Vec<TagSuggestion> = suggestions
            .into_iter()
            .map(|(tag, freq, score)| TagSuggestion {
                tag,
                frequency: freq,
                max_similarity: score,
            })
            .collect();

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&serde_json::json!({
                "success": true,
                "note": source_note.name,
                "current_tags": source_tags.iter().collect::<Vec<_>>(),
                "similar_notes_analyzed": similar_notes.len().saturating_sub(1),
                "suggestions": results
            }))
            .unwrap(),
        )]))
    }

    fn build_frontmatter(&self, params: &SaveParams) -> String {
        let mut fm = String::from("---\n");

        if let Some(t) = &params.note_type {
            fm.push_str(&format!("elysium_type: {}\n", t));
        }
        fm.push_str("elysium_status: active\n");
        if let Some(a) = &params.area {
            fm.push_str(&format!("elysium_area: {}\n", a));
        }
        if let Some(g) = &params.gist {
            fm.push_str(&format!("elysium_gist: >\n  {}\n", g));
            fm.push_str("elysium_gist_source: ai\n");
            fm.push_str(&format!(
                "elysium_gist_date: {}\n",
                chrono::Local::now().format("%Y-%m-%d")
            ));
        }

        // Handle tags: manual + auto-generated
        let final_tags = self.resolve_tags(params);
        if !final_tags.is_empty() {
            fm.push_str(&format!("elysium_tags: [{}]\n", final_tags.join(", ")));
        }

        if let Some(source) = &params.source {
            let sources: Vec<&str> = source.split(',').map(|s| s.trim()).collect();
            fm.push_str(&format!("elysium_source: [{}]\n", sources.join(", ")));
        }
        fm.push_str("---\n\n");
        fm
    }

    /// Resolve tags: combine manual tags with auto-generated tags
    fn resolve_tags(&self, params: &SaveParams) -> Vec<String> {
        let mut tags: Vec<String> = Vec::new();

        // Add manual tags first
        if let Some(manual_tags) = &params.tags {
            for tag in manual_tags.split(',').map(|t| t.trim()) {
                if !tag.is_empty() && !tags.contains(&tag.to_string()) {
                    tags.push(tag.to_string());
                }
            }
        }

        // Auto-generate tags if enabled and we have gist/title
        if params.auto_tag {
            let search_text = params.gist.as_deref().unwrap_or(&params.title);
            let auto_tags = self.suggest_tags(search_text, params.tag_limit, params.discover);

            for tag in auto_tags {
                if !tags.contains(&tag) {
                    tags.push(tag);
                }
            }

            // Limit total tags
            tags.truncate(params.tag_limit);
        }

        tags
    }
}

#[rmcp::tool_handler]
impl ServerHandler for VaultService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Second Brain Vault MCP Server. Provides semantic search and note access for Obsidian vault.".to_string()
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

/// Run the MCP server
pub async fn run_mcp_server(vault_path: PathBuf) -> Result<()> {
    use tokio::io::{stdin, stdout};

    let service = VaultService::new(vault_path);
    let transport = (stdin(), stdout());
    let server = service.serve(transport).await?;
    server.waiting().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::frontmatter::Frontmatter;
    use crate::mcp::params::{AuditParams, GetNoteParams, ListNotesParams, SearchParams};
    use crate::search::embedder::{Embedder, HtpEmbedder};
    use crate::search::plugin_index::{
        HnswIndex, IndexMeta, PluginSearchEngine, PLUGIN_INDEX_VERSION,
    };
    use rmcp::handler::server::wrapper::Parameters;
    use rmcp::model::RawContent;
    use serde::Deserialize;
    use std::collections::HashMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::tempdir;

    fn fixture_root() -> PathBuf {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/vault_small");
        let resolved = root.canonicalize().unwrap_or(root);
        assert!(
            resolved.exists(),
            "fixture vault not found at {}",
            resolved.display()
        );
        resolved
    }

    fn extract_text(result: &CallToolResult) -> String {
        result
            .content
            .iter()
            .find_map(|content| match &content.raw {
                RawContent::Text(text) => Some(text.text.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }

    #[derive(Deserialize)]
    struct BaselineSuite {
        baselines: Vec<SearchBaseline>,
    }

    #[derive(Deserialize)]
    struct SearchBaseline {
        mode: String,
        #[serde(rename = "maxRank")]
        max_rank: Option<usize>,
        queries: Vec<SearchBaselineQuery>,
    }

    #[derive(Deserialize)]
    struct SearchBaselineQuery {
        query: String,
        expected: String,
    }

    #[derive(serde::Serialize)]
    struct PluginNoteRecord {
        path: String,
        gist: String,
        mtime: u64,
        indexed: bool,
        fields: HashMap<String, serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tags: Option<Vec<String>>,
    }

    fn copy_fixture_notes(dest_root: &Path) {
        let fixture = fixture_root();
        for entry in fs::read_dir(fixture).expect("read fixture dir") {
            let entry = entry.expect("read fixture entry");
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
                continue;
            }
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .expect("fixture filename");
            let dest_path = dest_root.join(file_name);
            fs::copy(&path, &dest_path).expect("copy fixture note");
        }
    }

    fn write_plugin_index(vault_root: &Path) {
        let embedder = HtpEmbedder::new();

        let mut records = Vec::new();
        let mut ids = Vec::new();
        let mut vectors = Vec::new();

        let fixture = fixture_root();
        let mut fixture_count = 0;

        for entry in fs::read_dir(&fixture).expect("read fixture dir") {
            let entry = entry.expect("read fixture entry");
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
                continue;
            }

            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .expect("fixture filename")
                .to_string();

            let content = fs::read_to_string(&path).expect("read fixture note");
            let frontmatter = Frontmatter::parse(&content);
            let gist = frontmatter
                .as_ref()
                .and_then(|fm| fm.gist())
                .unwrap_or_else(|| file_name.trim_end_matches(".md"))
                .to_string();

            let fields = frontmatter
                .as_ref()
                .map(|fm| fm.to_json_map())
                .unwrap_or_default();
            let tags = frontmatter
                .as_ref()
                .and_then(|fm| fm.get_list("tags"))
                .cloned();

            ids.push(file_name.clone());
            vectors.push(embedder.embed(&gist).expect("embed gist"));

            records.push(PluginNoteRecord {
                path: file_name,
                gist,
                mtime: 0,
                indexed: true,
                fields,
                tags,
            });
            fixture_count += 1;
        }

        assert!(fixture_count > 0, "fixture notes missing");

        let sample_vector = vectors
            .first()
            .cloned()
            .expect("expected at least one vector");
        let hnsw = HnswIndex::from_vectors(ids, vectors);
        assert!(!hnsw.is_empty(), "constructed HNSW index is empty");
        let sanity = hnsw.search(&sample_vector, 3, 50);
        assert!(!sanity.is_empty(), "constructed HNSW search returned empty");
        let hnsw_data = bincode::serialize(&hnsw).expect("serialize hnsw");

        let meta = IndexMeta {
            embedding_mode: "htp".to_string(),
            dimension: embedder.dimension(),
            note_count: records.len(),
            index_size: hnsw_data.len(),
            exported_at: 0,
            version: PLUGIN_INDEX_VERSION,
        };

        let index_dir = vault_root.join(".obsidian/plugins/elysium/index");
        fs::create_dir_all(&index_dir).expect("create index dir");
        fs::write(index_dir.join("hnsw.bin"), hnsw_data).expect("write hnsw.bin");
        fs::write(
            index_dir.join("notes.json"),
            serde_json::to_string_pretty(&records).expect("serialize notes"),
        )
        .expect("write notes.json");
        fs::write(
            index_dir.join("meta.json"),
            serde_json::to_string_pretty(&meta).expect("serialize meta"),
        )
        .expect("write meta.json");
    }

    fn setup_vault_with_index() -> tempfile::TempDir {
        let temp = tempdir().expect("create temp dir");
        copy_fixture_notes(temp.path());
        write_plugin_index(temp.path());
        temp
    }

    #[tokio::test]
    async fn smoke_vault_list_notes() {
        let service = VaultService::new(fixture_root());
        let params = ListNotesParams {
            note_type: None,
            area: None,
            limit: 50,
            fields: Some("standard".to_string()),
        };

        let result = service
            .vault_list_notes(Parameters(params))
            .await
            .expect("vault_list_notes should succeed");
        assert_eq!(result.is_error, Some(false));

        let text = extract_text(&result);
        let items: Vec<serde_json::Value> =
            serde_json::from_str(&text).expect("list output should be JSON");
        assert_eq!(items.len(), 3);
        assert!(items.iter().all(|item| item.get("path").is_some()));
    }

    #[tokio::test]
    async fn smoke_vault_get_note() {
        let service = VaultService::new(fixture_root());
        let params = GetNoteParams {
            note: "alpha".to_string(),
            fields: Some("standard".to_string()),
        };

        let result = service
            .vault_get_note(Parameters(params))
            .await
            .expect("vault_get_note should succeed");

        let text = extract_text(&result);
        assert!(text.contains("## Metadata"));
        assert!(text.contains("# Alpha"));
    }

    #[tokio::test]
    async fn smoke_vault_status() {
        let service = VaultService::new(fixture_root());
        let result = service
            .vault_status()
            .await
            .expect("vault_status should succeed");

        let text = extract_text(&result);
        let status: serde_json::Value =
            serde_json::from_str(&text).expect("status output should be JSON");
        assert_eq!(status["total_notes"].as_u64(), Some(3));
    }

    #[tokio::test]
    async fn smoke_vault_audit() {
        let service = VaultService::new(fixture_root());
        let params = AuditParams {
            quick: true,
            verbose: false,
        };

        let result = service
            .vault_audit(Parameters(params))
            .await
            .expect("vault_audit should succeed");

        let text = extract_text(&result);
        let audit: serde_json::Value =
            serde_json::from_str(&text).expect("audit output should be JSON");
        let total_checks = audit["total_checks"].as_u64().unwrap_or(0);
        assert!(total_checks >= 2);
    }

    #[tokio::test]
    async fn smoke_vault_search_golden() {
        let temp = setup_vault_with_index();
        let service = VaultService::new(temp.path().to_path_buf());

        let baseline_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/golden/search_baselines.json");
        let baseline: BaselineSuite = serde_json::from_str(
            &fs::read_to_string(&baseline_path).expect("read search baseline"),
        )
        .expect("parse search baseline");

        let plugin_engine = PluginSearchEngine::load(temp.path()).expect("load plugin index");
        let sanity_results = plugin_engine
            .search("alpha", 3)
            .expect("plugin search should succeed");
        assert!(
            !sanity_results.is_empty(),
            "plugin search should return results"
        );

        for baseline_case in baseline.baselines {
            let max_rank = baseline_case.max_rank.unwrap_or(1).max(1);
            let limit = max_rank.max(3);

            for query in baseline_case.queries {
                let params = SearchParams {
                    query: query.query.clone(),
                    limit,
                    note_type: None,
                    area: None,
                    fields: Some("default".to_string()),
                    search_mode: Some(baseline_case.mode.clone()),
                };

                let result = service
                    .vault_search(Parameters(params))
                    .await
                    .expect("vault_search should succeed");
                let text = extract_text(&result);
                let results: Vec<serde_json::Value> =
                    serde_json::from_str(&text).expect("search output should be JSON");

                let top_slice = results.iter().take(max_rank);
                let top_paths: Vec<String> = top_slice
                    .filter_map(|item| item.get("path").and_then(|value| value.as_str()))
                    .map(|path| path.to_string())
                    .collect();

                assert!(
                    top_paths.contains(&query.expected),
                    "expected {} in top {} for mode {}. got {:?}",
                    query.expected,
                    max_rank,
                    baseline_case.mode,
                    top_paths
                );
            }
        }
    }
}
