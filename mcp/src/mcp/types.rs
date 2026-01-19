//! JSON output types for MCP tools

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct AuditCheckJson {
    pub id: String,
    pub name: String,
    pub status: String,
    pub errors: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warnings: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_list: Option<Vec<AuditErrorJson>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning_list: Option<Vec<AuditErrorJson>>,
}

#[derive(Debug, Serialize)]
pub struct AuditErrorJson {
    pub note: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct AuditResultJson {
    pub timestamp: String,
    pub total_checks: usize,
    pub passed: usize,
    pub failed: usize,
    pub checks: Vec<AuditCheckJson>,
}

/// Search result for JSON output
#[derive(Debug, Serialize)]
pub struct SearchResultJson {
    pub title: String,
    pub path: String,
    pub gist: Option<String>,
    pub note_type: Option<String>,
    pub area: Option<String>,
    pub score: f32,
}

/// Note info for JSON output
#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub struct NoteInfoJson {
    pub title: String,
    pub path: String,
    pub note_type: Option<String>,
    pub status: Option<String>,
    pub area: Option<String>,
    pub gist: Option<String>,
    pub tags: Vec<String>,
}
