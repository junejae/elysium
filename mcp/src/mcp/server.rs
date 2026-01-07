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

/// Parameters for vault_update_gist tool
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateGistParams {
    /// Note title or path
    #[schemars(description = "Note title or path to update")]
    pub note: String,
    /// New gist content
    #[schemars(description = "New gist summary (2-3 sentences)")]
    pub gist: String,
    /// Gist source (auto, ai, human)
    #[schemars(description = "Gist source: auto, ai, or human")]
    #[serde(default = "default_gist_source")]
    pub source: String,
}

fn default_gist_source() -> String {
    "ai".to_string()
}

/// Audit check result for JSON output
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
    model_path: PathBuf,
    tool_router: ToolRouter<Self>,
}

impl VaultService {
    pub fn new(vault_path: PathBuf) -> Self {
        let tools_path = vault_path.join(".opencode/tools");
        let db_path = tools_path.join("data/search.db");
        let model_path = tools_path.join("models/model.onnx"); // Not used with HTP

        Self {
            vault_path,
            db_path,
            model_path,
            tool_router: Self::tool_router(),
        }
    }

    fn get_engine(&self) -> Result<SearchEngine, McpError> {
        SearchEngine::new(&self.vault_path, &self.db_path, &self.model_path)
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

    /// Get full content of a specific note
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

    /// Get vault health score
    #[tool(
        description = "Get Second Brain Vault health score (0-100) based on schema compliance, gist coverage, and link integrity."
    )]
    async fn vault_health(&self) -> Result<CallToolResult, McpError> {
        let vault_paths = self.get_vault_paths();
        let notes = collect_all_notes(&vault_paths);

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
            "score": health_score,
            "total_notes": total,
            "gist_coverage": format!("{:.0}%", if total > 0 { (with_gist as f64 / total as f64) * 100.0 } else { 0.0 }),
            "type_coverage": format!("{:.0}%", if total > 0 { (with_type as f64 / total as f64) * 100.0 } else { 0.0 }),
            "area_coverage": format!("{:.0}%", if total > 0 { (with_area as f64 / total as f64) * 100.0 } else { 0.0 }),
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&output).unwrap_or_default(),
        )]))
    }

    /// Get vault status summary
    #[tool(
        description = "Get Second Brain Vault status summary including note counts by type and area."
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

        let output = serde_json::json!({
            "total_notes": notes.len(),
            "by_type": by_type,
            "by_area": by_area,
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
            // Folder-type match check
            let folder_type_check = self.check_folder_type(&notes, verbose);
            checks.push(folder_type_check);

            // Gist coverage check
            let gist_check = self.check_gist(&notes, verbose);
            checks.push(gist_check);

            // Tag usage check
            let tags_check = self.check_tags(&notes, verbose);
            checks.push(tags_check);

            // Orphan notes check
            let orphans_check = self.check_orphans(&notes, &note_names, verbose);
            checks.push(orphans_check);
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

    /// Get inbox file content for AI processing
    #[tool(
        description = "Get the content of the inbox file for processing. Returns the raw content of the quick-capture inbox file."
    )]
    async fn vault_get_inbox(&self) -> Result<CallToolResult, McpError> {
        let vault_paths = self.get_vault_paths();
        let inbox_path = vault_paths.config.resolve_paths(&self.vault_path).inbox;

        if !inbox_path.exists() {
            return Ok(CallToolResult::success(vec![Content::text(
                serde_json::json!({
                    "exists": false,
                    "path": inbox_path.to_string_lossy(),
                    "content": null
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
            "lines": content.lines().count()
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

    /// Get notes with stale gists (gist_date < file modified time)
    #[tool(
        description = "Get list of notes where gist is outdated (gist_date older than file modification time). Useful for AI to identify notes needing gist refresh."
    )]
    async fn vault_get_stale_gists(&self) -> Result<CallToolResult, McpError> {
        let vault_paths = self.get_vault_paths();
        let notes = collect_all_notes(&vault_paths);

        let mut stale_notes: Vec<serde_json::Value> = Vec::new();

        let gist_date_re = regex::Regex::new(r"(?m)^gist_date:\s*(\d{4}-\d{2}-\d{2})").unwrap();

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
                            stale_notes.push(serde_json::json!({
                                "title": note.name,
                                "path": note.path.to_string_lossy(),
                                "gist_date": gist_date.to_string(),
                                "modified_date": modified_date.to_string(),
                                "current_gist": note.gist()
                            }));
                        }
                    }
                }
            }
        }

        let output = serde_json::json!({
            "count": stale_notes.len(),
            "notes": stale_notes
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&output).unwrap_or_default(),
        )]))
    }

    /// Update gist for a specific note
    #[tool(
        description = "Update the gist field for a note. Sets gist, gist_source (auto/ai/human), and gist_date in frontmatter."
    )]
    async fn vault_update_gist(
        &self,
        params: Parameters<UpdateGistParams>,
    ) -> Result<CallToolResult, McpError> {
        let vault_paths = self.get_vault_paths();
        let notes = collect_all_notes(&vault_paths);
        let note_name = &params.0.note;

        let found = notes.into_iter().find(|n| {
            n.name == *note_name
                || n.path.to_string_lossy().contains(note_name)
                || n.path.file_stem().map(|s| s.to_string_lossy().to_string())
                    == Some(note_name.clone())
        });

        match found {
            Some(note) => {
                let content = std::fs::read_to_string(&note.path).map_err(|e| {
                    McpError::internal_error(format!("Failed to read note: {}", e), None)
                })?;

                let today = chrono::Local::now().format("%Y-%m-%d").to_string();
                let new_content = self.update_frontmatter_gist(
                    &content,
                    &params.0.gist,
                    &params.0.source,
                    &today,
                );

                std::fs::write(&note.path, &new_content).map_err(|e| {
                    McpError::internal_error(format!("Failed to write note: {}", e), None)
                })?;

                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::json!({
                        "success": true,
                        "note": note.name,
                        "gist": params.0.gist,
                        "source": params.0.source,
                        "date": today
                    })
                    .to_string(),
                )]))
            }
            None => Ok(CallToolResult::success(vec![Content::text(
                serde_json::json!({
                    "success": false,
                    "error": format!("Note not found: {}", note_name)
                })
                .to_string(),
            )])),
        }
    }
}

impl VaultService {
    fn update_frontmatter_gist(
        &self,
        content: &str,
        gist: &str,
        source: &str,
        date: &str,
    ) -> String {
        let fm_regex = regex::Regex::new(r"^---\s*\n([\s\S]*?)\n---").unwrap();

        if let Some(caps) = fm_regex.captures(content) {
            let fm_content = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let after_fm = &content[caps.get(0).unwrap().end()..];

            let mut new_fm = String::new();
            let mut gist_added = false;
            let mut source_added = false;
            let mut date_added = false;

            for line in fm_content.lines() {
                if line.starts_with("gist:") {
                    new_fm.push_str(&format!("gist: >\n  {}\n", gist));
                    gist_added = true;
                } else if line.starts_with("gist_source:") {
                    new_fm.push_str(&format!("gist_source: {}\n", source));
                    source_added = true;
                } else if line.starts_with("gist_date:") {
                    new_fm.push_str(&format!("gist_date: {}\n", date));
                    date_added = true;
                } else if line.starts_with("  ") && gist_added && !line.contains(':') {
                    continue;
                } else {
                    new_fm.push_str(line);
                    new_fm.push('\n');
                }
            }

            if !gist_added {
                new_fm.push_str(&format!("gist: >\n  {}\n", gist));
            }
            if !source_added {
                new_fm.push_str(&format!("gist_source: {}\n", source));
            }
            if !date_added {
                new_fm.push_str(&format!("gist_date: {}\n", date));
            }

            format!("---\n{}---{}", new_fm, after_fm)
        } else {
            format!(
                "---\ngist: >\n  {}\ngist_source: {}\ngist_date: {}\n---\n{}",
                gist, source, date, content
            )
        }
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

    fn check_folder_type(
        &self,
        notes: &[crate::core::note::Note],
        verbose: bool,
    ) -> AuditCheckJson {
        let mut errors = Vec::new();
        for note in notes {
            if !note.check_folder_type_match() {
                errors.push(AuditErrorJson {
                    note: note.name.clone(),
                    message: format!(
                        "Type '{}' in folder '{}'",
                        note.note_type().unwrap_or("none"),
                        note.folder()
                    ),
                });
            }
        }

        AuditCheckJson {
            id: "folder_type".to_string(),
            name: "Folder-Type Match".to_string(),
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
}

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
