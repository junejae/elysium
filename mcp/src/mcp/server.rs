//! Vault MCP Server implementation

use anyhow::Result;
use rmcp::{
    handler::server::{tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, ServerInfo},
    tool, tool_router, ErrorData as McpError, ServerHandler, ServiceExt,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::core::note::{collect_all_notes, collect_note_names};
use crate::core::paths::VaultPaths;
use crate::core::schema::SchemaValidator;
use crate::search::engine::SearchEngine;
use std::collections::HashSet;

/// Parameters for vault_search tool
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchParams {
    /// Natural language search query (e.g., "GPU memory sharing methods")
    #[schemars(description = "Natural language search query")]
    pub query: String,
    /// Maximum number of results to return (default: 5)
    #[schemars(description = "Maximum number of results (default: 5)")]
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    5
}

/// Parameters for vault_get_note tool
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetNoteParams {
    /// Note title (e.g., "GPU 기술 허브")
    #[schemars(description = "Note title to retrieve")]
    pub note: String,
}

/// Parameters for vault_list_notes tool
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListNotesParams {
    /// Filter by note type (note, term, project, log)
    #[schemars(description = "Filter by type: note, term, project, log")]
    #[serde(default)]
    pub note_type: Option<String>,
    /// Filter by area (work, tech, life, career, learning, reference)
    #[schemars(description = "Filter by area: work, tech, life, career, learning, reference")]
    #[serde(default)]
    pub area: Option<String>,
    /// Maximum number of results (default: 50)
    #[schemars(description = "Maximum results (default: 50)")]
    #[serde(default = "default_list_limit")]
    pub limit: usize,
}

fn default_list_limit() -> usize {
    50
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RelatedParams {
    #[schemars(description = "Note title to find related notes for")]
    pub note: String,
    #[schemars(description = "Maximum number of results (default: 10)")]
    #[serde(default = "default_related_limit")]
    pub limit: usize,
    #[schemars(description = "Boost notes with same type as source")]
    #[serde(default)]
    pub boost_type: bool,
    #[schemars(description = "Boost notes with same area as source")]
    #[serde(default)]
    pub boost_area: bool,
}

fn default_related_limit() -> usize {
    10
}

/// Parameters for vault_audit tool
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AuditParams {
    /// Quick mode: schema + wikilinks only
    #[schemars(description = "Quick mode: schema + wikilinks only")]
    #[serde(default)]
    pub quick: bool,

    /// Include detailed error messages
    #[schemars(description = "Include detailed error list per check")]
    #[serde(default)]
    pub verbose: bool,
}

/// Parameters for unified vault_save tool
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SaveParams {
    /// Note title (used as filename for create, or to find existing note)
    #[schemars(description = "Note title (filename for create, or search key for update/append)")]
    pub title: String,

    /// Note content (markdown body, or memo for inbox strategy)
    #[schemars(description = "Note content (markdown body)")]
    pub content: String,

    /// Save strategy: create, update, append, inbox, smart
    #[schemars(
        description = "Save strategy: 'create' (new note), 'update' (overwrite existing), 'append' (add to existing), 'inbox' (quick capture), 'smart' (auto-detect duplicates)"
    )]
    #[serde(default = "default_strategy")]
    pub strategy: String,

    /// Note type: note, term, project, log
    #[schemars(description = "Note type: note, term, project, log")]
    #[serde(default)]
    pub note_type: Option<String>,

    /// Note area: work, tech, life, career, learning, reference
    #[schemars(description = "Note area: work, tech, life, career, learning, reference")]
    #[serde(default)]
    pub area: Option<String>,

    /// Tags (comma-separated)
    #[schemars(description = "Tags (comma-separated, e.g., 'gpu, cuda, nvidia')")]
    #[serde(default)]
    pub tags: Option<String>,

    /// Gist summary (2-3 sentences for semantic search)
    #[schemars(description = "Gist summary (2-3 sentences for semantic search)")]
    #[serde(default)]
    pub gist: Option<String>,

    /// Source URL (for web research notes)
    #[schemars(description = "Source URL (for notes from web research)")]
    #[serde(default)]
    pub source: Option<String>,

    /// Similarity threshold for smart strategy (default: 0.7)
    #[schemars(description = "Similarity threshold for smart strategy (0.0-1.0, default: 0.7)")]
    #[serde(default = "default_similarity_threshold")]
    pub similarity_threshold: Option<f32>,
}

fn default_strategy() -> String {
    "create".to_string()
}

fn default_similarity_threshold() -> Option<f32> {
    Some(0.7)
}

#[derive(Debug, Serialize)]
struct AuditCheckJson {
    id: String,
    name: String,
    status: String,
    errors: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_list: Option<Vec<AuditErrorJson>>,
}

#[derive(Debug, Serialize)]
struct AuditErrorJson {
    note: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct AuditResultJson {
    timestamp: String,
    total_checks: usize,
    passed: usize,
    failed: usize,
    checks: Vec<AuditCheckJson>,
}

/// Search result for JSON output
#[derive(Debug, Serialize)]
struct SearchResultJson {
    title: String,
    path: String,
    gist: Option<String>,
    note_type: Option<String>,
    area: Option<String>,
    score: f32,
}

/// Note info for JSON output
#[derive(Debug, Serialize)]
struct NoteInfoJson {
    title: String,
    path: String,
    note_type: Option<String>,
    status: Option<String>,
    area: Option<String>,
    gist: Option<String>,
    tags: Vec<String>,
}

/// Vault MCP Service
#[derive(Clone)]
pub struct VaultService {
    vault_path: PathBuf,
    db_path: PathBuf,
    tool_router: ToolRouter<Self>,
}

impl VaultService {
    pub fn new(vault_path: PathBuf) -> Self {
        let tools_path = vault_path.join(".opencode/tools");
        let db_path = tools_path.join("data/search.db");

        Self {
            vault_path,
            db_path,
            tool_router: Self::tool_router(),
        }
    }

    fn get_engine(&self) -> Result<SearchEngine, McpError> {
        SearchEngine::new(&self.vault_path, &self.db_path)
            .map_err(|e| McpError::internal_error(format!("Failed to create engine: {}", e), None))
    }

    fn get_vault_paths(&self) -> VaultPaths {
        VaultPaths::from_root(self.vault_path.clone())
    }

    fn get_schema_validator(&self) -> SchemaValidator {
        let vault_paths = self.get_vault_paths();
        SchemaValidator::from_config(&vault_paths.config.schema)
    }
}

#[tool_router]
impl VaultService {
    /// Search notes using semantic similarity
    #[tool(
        description = "Search Second Brain Vault using semantic similarity. Returns notes with similar meaning to the query based on gist field embeddings."
    )]
    async fn vault_search(
        &self,
        params: Parameters<SearchParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut engine = self.get_engine()?;
        // Clamp limit: default 5, max 100 (DoS prevention)
        let limit = params.0.limit.max(1).min(100);
        let limit = if limit == 1 && params.0.limit == 0 {
            5
        } else {
            limit
        };

        let results = engine
            .search(&params.0.query, limit)
            .map_err(|e| McpError::internal_error(format!("Search failed: {}", e), None))?;

        let json_results: Vec<SearchResultJson> = results
            .into_iter()
            .map(|r| SearchResultJson {
                title: r.title,
                path: r.path,
                gist: r.gist,
                note_type: r.note_type,
                area: r.area,
                score: r.score,
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
        use crate::search::engine::BoostOptions;

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

        let mut engine = self.get_engine()?;
        let limit = params.0.limit.max(1).min(50);

        let results = if params.0.boost_type || params.0.boost_area {
            let boost = BoostOptions::from_source(
                source_note.note_type(),
                source_note.area(),
                params.0.boost_type,
                params.0.boost_area,
            );
            engine
                .search_with_boost(gist, limit + 1, &boost)
                .map_err(|e| McpError::internal_error(format!("Search failed: {}", e), None))?
        } else {
            engine
                .search(gist, limit + 1)
                .map_err(|e| McpError::internal_error(format!("Search failed: {}", e), None))?
        };

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

                let info = NoteInfoJson {
                    title: n.name.clone(),
                    path: n.path.to_string_lossy().to_string(),
                    note_type: n.note_type().map(String::from),
                    status: n.status().map(String::from),
                    area: n.area().map(String::from),
                    gist: n.gist().map(String::from),
                    tags: n.tags(),
                };

                let output = format!(
                    "## Metadata\n```json\n{}\n```\n\n## Content\n{}",
                    serde_json::to_string_pretty(&info).unwrap_or_default(),
                    content
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

        let filtered: Vec<NoteInfoJson> = notes
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
            .map(|n| NoteInfoJson {
                title: n.name.clone(),
                path: n.path.to_string_lossy().to_string(),
                note_type: n.note_type().map(String::from),
                status: n.status().map(String::from),
                area: n.area().map(String::from),
                gist: n.gist().map(String::from),
                tags: n.tags(),
            })
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

        let mut checks = Vec::new();

        // Schema check
        let schema_check = self.check_schema(&notes, verbose);
        checks.push(schema_check);

        // Wikilinks check
        let wikilinks_check = self.check_wikilinks(&notes, &note_names, verbose);
        checks.push(wikilinks_check);

        if !quick {
            // Gist coverage check
            let gist_check = self.check_gist(&notes, verbose);
            checks.push(gist_check);

            // Tag usage check
            let tags_check = self.check_tags(&notes, verbose);
            checks.push(tags_check);

            // Orphan notes check
            let orphans_check = self.check_orphans(&notes, &note_names, verbose);
            checks.push(orphans_check);

            // Stale gists check
            let stale_gists_check = self.check_stale_gists(&notes, verbose);
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

impl VaultService {
    fn get_target_folder(&self, note_type: Option<&str>) -> PathBuf {
        let vault_paths = self.get_vault_paths();
        let folders = &vault_paths.config.folders;

        let folder = match note_type {
            Some("project") => &folders.projects,
            _ => &folders.notes,
        };
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
        let full_content = format!("{}# {}\n\n{}", frontmatter, params.title, params.content);

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
                let full_content =
                    format!("{}# {}\n\n{}", frontmatter, params.title, params.content);

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
        if let Some(tags) = &params.tags {
            let tag_list: Vec<&str> = tags.split(',').map(|t| t.trim()).collect();
            fm.push_str(&format!("elysium_tags: [{}]\n", tag_list.join(", ")));
        }
        if let Some(source) = &params.source {
            fm.push_str(&format!("source: {}\n", source));
        }
        fm.push_str("---\n\n");
        fm
    }
}

// Audit helper methods
impl VaultService {
    fn check_schema(&self, notes: &[crate::core::note::Note], verbose: bool) -> AuditCheckJson {
        let validator = self.get_schema_validator();
        let mut errors = Vec::new();
        for note in notes {
            let violations = note.validate_schema_with_config(&validator);
            for violation in violations {
                errors.push(AuditErrorJson {
                    note: note.name.clone(),
                    message: format!("{:?}", violation),
                });
            }
        }

        AuditCheckJson {
            id: "schema".to_string(),
            name: "YAML Schema".to_string(),
            status: if errors.is_empty() { "pass" } else { "fail" }.to_string(),
            errors: errors.len(),
            details: None,
            error_list: if verbose && !errors.is_empty() {
                Some(errors)
            } else {
                None
            },
        }
    }

    fn check_wikilinks(
        &self,
        notes: &[crate::core::note::Note],
        note_names: &HashSet<String>,
        verbose: bool,
    ) -> AuditCheckJson {
        let mut errors = Vec::new();
        for note in notes {
            for link in note.wikilinks() {
                if !note_names.contains(&link) {
                    errors.push(AuditErrorJson {
                        note: note.name.clone(),
                        message: format!("Broken link: [[{}]]", link),
                    });
                }
            }
        }

        AuditCheckJson {
            id: "wikilinks".to_string(),
            name: "Wikilinks".to_string(),
            status: if errors.is_empty() { "pass" } else { "fail" }.to_string(),
            errors: errors.len(),
            details: None,
            error_list: if verbose && !errors.is_empty() {
                Some(errors)
            } else {
                None
            },
        }
    }

    fn check_gist(&self, notes: &[crate::core::note::Note], verbose: bool) -> AuditCheckJson {
        let mut errors = Vec::new();
        for note in notes {
            if note.gist().is_none() {
                errors.push(AuditErrorJson {
                    note: note.name.clone(),
                    message: "Missing gist".to_string(),
                });
            }
        }

        let total = notes.len();
        let missing = errors.len();
        let coverage = if total > 0 {
            ((total - missing) as f64 / total as f64 * 100.0).round() as usize
        } else {
            100
        };

        AuditCheckJson {
            id: "gist".to_string(),
            name: "Gist Coverage".to_string(),
            status: if missing == 0 { "pass" } else { "fail" }.to_string(),
            errors: missing,
            details: Some(format!("{}% coverage ({} missing)", coverage, missing)),
            error_list: if verbose && !errors.is_empty() {
                Some(errors)
            } else {
                None
            },
        }
    }

    fn check_tags(&self, notes: &[crate::core::note::Note], verbose: bool) -> AuditCheckJson {
        let mut errors = Vec::new();
        for note in notes {
            if note.tags().is_empty() {
                errors.push(AuditErrorJson {
                    note: note.name.clone(),
                    message: "No tags".to_string(),
                });
            }
        }

        let total = notes.len();
        let without_tags = errors.len();
        let ratio = if total > 0 {
            without_tags as f64 / total as f64
        } else {
            0.0
        };

        AuditCheckJson {
            id: "tags".to_string(),
            name: "Tag Usage".to_string(),
            status: if ratio < 0.3 { "pass" } else { "fail" }.to_string(),
            errors: without_tags,
            details: Some(format!("{:.0}% notes without tags", ratio * 100.0)),
            error_list: if verbose && !errors.is_empty() {
                Some(errors)
            } else {
                None
            },
        }
    }

    fn check_orphans(
        &self,
        notes: &[crate::core::note::Note],
        note_names: &HashSet<String>,
        verbose: bool,
    ) -> AuditCheckJson {
        let mut linked: HashSet<String> = HashSet::new();
        for note in notes {
            for link in note.wikilinks() {
                if note_names.contains(&link) {
                    linked.insert(link);
                }
            }
        }

        let mut errors = Vec::new();
        for name in note_names {
            if !linked.contains(name) {
                errors.push(AuditErrorJson {
                    note: name.clone(),
                    message: "Orphan note (no incoming links)".to_string(),
                });
            }
        }

        let total = notes.len();
        let orphans = errors.len();
        let ratio = if total > 0 {
            orphans as f64 / total as f64
        } else {
            0.0
        };

        AuditCheckJson {
            id: "orphans".to_string(),
            name: "Orphan Notes".to_string(),
            status: if ratio < 0.3 { "pass" } else { "fail" }.to_string(),
            errors: orphans,
            details: Some(format!("{} orphan notes ({:.0}%)", orphans, ratio * 100.0)),
            error_list: if verbose && !errors.is_empty() {
                Some(errors)
            } else {
                None
            },
        }
    }

    fn check_stale_gists(
        &self,
        notes: &[crate::core::note::Note],
        verbose: bool,
    ) -> AuditCheckJson {
        let mut errors = Vec::new();
        let gist_date_re =
            regex::Regex::new(r"(?m)^elysium_gist_date:\s*(\d{4}-\d{2}-\d{2})").unwrap();

        for note in notes {
            let gist_date = note
                .frontmatter
                .as_ref()
                .and_then(|fm| gist_date_re.captures(&fm.raw))
                .and_then(|caps| caps.get(1))
                .and_then(|m| chrono::NaiveDate::parse_from_str(m.as_str(), "%Y-%m-%d").ok());

            if let Some(gist_date) = gist_date {
                if let Ok(metadata) = std::fs::metadata(&note.path) {
                    if let Ok(modified) = metadata.modified() {
                        let modified_date =
                            chrono::DateTime::<chrono::Local>::from(modified).date_naive();
                        if gist_date < modified_date {
                            errors.push(AuditErrorJson {
                                note: note.name.clone(),
                                message: format!("Stale gist: {} < {}", gist_date, modified_date),
                            });
                        }
                    }
                }
            }
        }

        AuditCheckJson {
            id: "stale_gists".to_string(),
            name: "Stale Gists".to_string(),
            status: if errors.is_empty() { "pass" } else { "warn" }.to_string(),
            errors: errors.len(),
            details: Some(format!("{} notes with outdated gists", errors.len())),
            error_list: if verbose && !errors.is_empty() {
                Some(errors)
            } else {
                None
            },
        }
    }
}

#[rmcp::tool_handler]
impl ServerHandler for VaultService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Second Brain Vault MCP Server. Provides semantic search and note access for Obsidian vault.".to_string()
            ),
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
