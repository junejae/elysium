//! Helper functions for MCP tools

use std::collections::HashMap;

use crate::core::frontmatter::{DEFAULT_FIELDS, STANDARD_FIELDS};
use crate::core::note::Note;

/// Resolve fields parameter to actual field list
/// Returns (field_list, is_all)
pub fn resolve_fields(fields_param: &Option<String>) -> (Vec<&'static str>, bool) {
    match fields_param.as_deref() {
        None | Some("default") => (DEFAULT_FIELDS.to_vec(), false),
        Some("standard") => (STANDARD_FIELDS.to_vec(), false),
        Some("all") => (vec![], true), // empty means all fields
        Some(custom) => {
            let fields: Vec<&str> = custom.split(',').map(|s| s.trim()).collect();
            // Convert to static strings by matching known fields
            let known_fields = [
                "title",
                "path",
                "type",
                "status",
                "area",
                "gist",
                "tags",
                "source",
                "gist_source",
                "gist_date",
            ];
            let filtered: Vec<&'static str> = fields
                .iter()
                .filter_map(|f| known_fields.iter().find(|&&k| k == *f).copied())
                .collect();
            (filtered, false)
        }
    }
}

/// Build dynamic JSON output for a note based on requested fields
pub fn build_note_json(
    note: &Note,
    fields_param: &Option<String>,
) -> HashMap<String, serde_json::Value> {
    let (requested_fields, is_all) = resolve_fields(fields_param);

    let mut result: HashMap<String, serde_json::Value> = HashMap::new();

    // Always include title and path
    result.insert(
        "title".to_string(),
        serde_json::Value::String(note.name.clone()),
    );
    result.insert(
        "path".to_string(),
        serde_json::Value::String(note.path.display().to_string()),
    );

    if is_all {
        // Include all frontmatter fields
        if let Some(fm) = &note.frontmatter {
            for (key, value) in fm.to_json_map() {
                result.insert(key, value);
            }
        }
    } else {
        // Include only requested fields from frontmatter
        if let Some(fm) = &note.frontmatter {
            let fm_fields = fm.to_json_map();
            for field in &requested_fields {
                if *field == "title" || *field == "path" {
                    continue; // Already added
                }
                if let Some(value) = fm_fields.get(*field) {
                    result.insert(field.to_string(), value.clone());
                }
            }
        }
    }

    result
}
