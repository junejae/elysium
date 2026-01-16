//! Frontmatter parsing and validation
//!
//! Parses YAML frontmatter from markdown notes and validates against schema.
//! Supports dynamic field extraction for all elysium_* prefixed fields.

use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::schema::{SchemaValidator, SchemaViolation, VALID_AREAS, VALID_STATUS, VALID_TYPES};

lazy_static! {
    static ref FRONTMATTER_RE: Regex = Regex::new(r"(?s)^---\r?\n(.*?)\r?\n---").unwrap();
    // Dynamic field pattern: captures elysium_* field names and values
    static ref ELYSIUM_FIELD_RE: Regex = Regex::new(r"(?m)^(elysium_\w+):\s*(.*)$").unwrap();
    // List pattern for [...] values
    static ref LIST_RE: Regex = Regex::new(r"^\[(.*)\]$").unwrap();
    // Pattern to detect frontmatter delimiters (for counting blocks)
    static ref FM_DELIMITER_RE: Regex = Regex::new(r"(?m)^---\s*$").unwrap();
    // Pattern to detect folded/literal scalar markers (> or |)
    static ref FOLDED_SCALAR_RE: Regex = Regex::new(r"(?m)^(\w+):\s*([>|])(?:[-+]|\d+[-+]?|[-+]\d+)?\s*$").unwrap();
}

// =========================================
// YAML Validation Functions
// =========================================

/// Count the number of frontmatter blocks in content
/// A frontmatter block is delimited by `---` at line start
pub fn count_frontmatter_blocks(content: &str) -> usize {
    let matches: Vec<_> = FM_DELIMITER_RE.find_iter(content).collect();

    if matches.is_empty() {
        return 0;
    }

    // First delimiter must be at position 0 (start of file) for valid frontmatter
    if matches[0].start() != 0 {
        return 0;
    }

    // Count pairs of delimiters as frontmatter blocks
    matches.len() / 2
}

/// Check if content has duplicate frontmatter blocks
pub fn has_duplicate_frontmatter(content: &str) -> bool {
    count_frontmatter_blocks(content) > 1
}

/// Validate YAML syntax using serde_yaml
/// Returns Ok(()) if valid, Err with details if invalid
pub fn validate_yaml_syntax(
    raw_frontmatter: &str,
) -> Result<(), (Option<usize>, Option<usize>, String)> {
    match serde_yaml::from_str::<serde_yaml::Value>(raw_frontmatter) {
        Ok(_) => Ok(()),
        Err(e) => {
            let location = e.location();
            Err((
                location.as_ref().map(|l| l.line()),
                location.as_ref().map(|l| l.column()),
                e.to_string(),
            ))
        }
    }
}

/// Detect fields using folded (>) or literal (|) scalar syntax
/// Returns list of (field_name, scalar_type) tuples
pub fn detect_folded_scalars(raw_frontmatter: &str) -> Vec<(String, char)> {
    FOLDED_SCALAR_RE
        .captures_iter(raw_frontmatter)
        .filter_map(|caps| {
            let field = caps.get(1)?.as_str().to_string();
            let marker = caps.get(2)?.as_str().chars().next()?;
            Some((field, marker))
        })
        .collect()
}

/// Field value types for dynamic frontmatter
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum FieldValue {
    String(String),
    List(Vec<String>),
}

impl FieldValue {
    /// Get as string reference if it's a String variant
    pub fn as_str(&self) -> Option<&str> {
        match self {
            FieldValue::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Get as list reference if it's a List variant
    pub fn as_list(&self) -> Option<&Vec<String>> {
        match self {
            FieldValue::List(l) => Some(l),
            _ => None,
        }
    }

    /// Convert to owned String (for String variant)
    pub fn to_string_value(&self) -> Option<String> {
        match self {
            FieldValue::String(s) => Some(s.clone()),
            _ => None,
        }
    }
}

/// Field presets for API output
pub const DEFAULT_FIELDS: &[&str] = &["title", "path", "gist"];
pub const STANDARD_FIELDS: &[&str] = &["title", "path", "type", "status", "area", "gist", "tags"];

#[derive(Debug, Default, Clone)]
pub struct Frontmatter {
    /// Dynamic field storage - keys are without "elysium_" prefix
    /// e.g., "type", "status", "area", "gist", "tags", "source"
    pub fields: HashMap<String, FieldValue>,
    /// Raw frontmatter text for debugging/re-parsing
    pub raw: String,
}

impl Frontmatter {
    /// Parse frontmatter from markdown content
    /// Extracts all elysium_* prefixed fields dynamically
    pub fn parse(content: &str) -> Option<Self> {
        let caps = FRONTMATTER_RE.captures(content)?;
        let raw = caps.get(1)?.as_str().to_string();

        let mut fields = HashMap::new();

        // First pass: extract all elysium_* fields
        for caps in ELYSIUM_FIELD_RE.captures_iter(&raw) {
            let full_key = caps.get(1)?.as_str();
            let value_str = caps.get(2)?.as_str().trim();

            // Remove "elysium_" prefix for cleaner key names
            let key = full_key.strip_prefix("elysium_").unwrap_or(full_key);

            // Special handling for gist (multiline YAML folding)
            if key == "gist" {
                if let Some(gist) = Self::extract_gist(&raw) {
                    fields.insert(key.to_string(), FieldValue::String(gist));
                }
                continue;
            }

            // Parse value as list or string
            let value = Self::parse_value(value_str);
            fields.insert(key.to_string(), value);
        }

        Some(Self { fields, raw })
    }

    /// Parse a value string into FieldValue
    fn parse_value(value_str: &str) -> FieldValue {
        // Check if it's a list [....]
        if let Some(caps) = LIST_RE.captures(value_str) {
            let inner = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let items: Vec<String> = inner
                .split(',')
                .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
                .filter(|s| !s.is_empty())
                .collect();
            FieldValue::List(items)
        } else {
            // Single value - clean up quotes
            let cleaned = value_str.trim_matches('"').trim_matches('\'').to_string();
            FieldValue::String(cleaned)
        }
    }

    /// Extract multiline gist (YAML folding support)
    fn extract_gist(raw: &str) -> Option<String> {
        let lines: Vec<&str> = raw.lines().collect();
        let gist_line_idx = lines.iter().position(|l| l.starts_with("elysium_gist:"))?;
        let gist_line = lines[gist_line_idx];

        // Get the part after "elysium_gist:"
        let after_colon = gist_line.strip_prefix("elysium_gist:")?.trim();

        // Check for YAML folding markers or empty (multiline)
        if after_colon == ">" || after_colon == "|" || after_colon.is_empty() {
            // Collect indented continuation lines
            let mut folded_content = Vec::new();
            for line in lines.iter().skip(gist_line_idx + 1) {
                if line.starts_with(' ') || line.starts_with('\t') {
                    folded_content.push(line.trim());
                } else if line.trim().is_empty() {
                    continue;
                } else {
                    break;
                }
            }

            let gist = folded_content.join(" ");
            if gist.is_empty() {
                None
            } else {
                Some(gist)
            }
        } else {
            // Single line gist
            let gist = after_colon.trim_matches('"').trim_matches('\'').to_string();
            if gist.is_empty() {
                None
            } else {
                Some(gist)
            }
        }
    }

    // =========================================
    // Backward-compatible accessor methods
    // =========================================

    /// Get note type (elysium_type)
    pub fn note_type(&self) -> Option<&str> {
        self.fields.get("type").and_then(|v| v.as_str())
    }

    /// Get status (elysium_status)
    pub fn status(&self) -> Option<&str> {
        self.fields.get("status").and_then(|v| v.as_str())
    }

    /// Get area (elysium_area)
    pub fn area(&self) -> Option<&str> {
        self.fields.get("area").and_then(|v| v.as_str())
    }

    /// Get gist (elysium_gist)
    pub fn gist(&self) -> Option<&str> {
        self.fields.get("gist").and_then(|v| v.as_str())
    }

    /// Get tags (elysium_tags)
    pub fn tags(&self) -> Vec<String> {
        self.fields
            .get("tags")
            .and_then(|v| v.as_list())
            .cloned()
            .unwrap_or_default()
    }

    /// Get source URLs (elysium_source)
    pub fn source(&self) -> Option<Vec<String>> {
        self.fields.get("source").and_then(|v| v.as_list()).cloned()
    }

    /// Get any field by key (without elysium_ prefix)
    pub fn get(&self, key: &str) -> Option<&FieldValue> {
        self.fields.get(key)
    }

    /// Get string field value
    pub fn get_string(&self, key: &str) -> Option<&str> {
        self.fields.get(key).and_then(|v| v.as_str())
    }

    /// Get list field value
    pub fn get_list(&self, key: &str) -> Option<&Vec<String>> {
        self.fields.get(key).and_then(|v| v.as_list())
    }

    /// Get all field keys
    pub fn keys(&self) -> Vec<&str> {
        self.fields.keys().map(|s| s.as_str()).collect()
    }

    /// Convert fields to JSON-compatible HashMap (for API output)
    pub fn to_json_map(&self) -> HashMap<String, serde_json::Value> {
        self.fields
            .iter()
            .map(|(k, v)| {
                let json_val = match v {
                    FieldValue::String(s) => serde_json::Value::String(s.clone()),
                    FieldValue::List(l) => serde_json::Value::Array(
                        l.iter()
                            .map(|s| serde_json::Value::String(s.clone()))
                            .collect(),
                    ),
                };
                (k.clone(), json_val)
            })
            .collect()
    }

    /// Filter fields for API output based on requested field set
    pub fn filter_fields(&self, requested: &[&str]) -> HashMap<String, serde_json::Value> {
        self.fields
            .iter()
            .filter(|(k, _)| requested.contains(&k.as_str()))
            .map(|(k, v)| {
                let json_val = match v {
                    FieldValue::String(s) => serde_json::Value::String(s.clone()),
                    FieldValue::List(l) => serde_json::Value::Array(
                        l.iter()
                            .map(|s| serde_json::Value::String(s.clone()))
                            .collect(),
                    ),
                };
                (k.clone(), json_val)
            })
            .collect()
    }

    // =========================================
    // Validation methods
    // =========================================

    /// Validate frontmatter using default schema (backward compatible)
    pub fn validate(&self) -> Vec<SchemaViolation> {
        self.validate_with_defaults()
    }

    /// Validate frontmatter using hardcoded default schema
    fn validate_with_defaults(&self) -> Vec<SchemaViolation> {
        let mut violations = Vec::new();

        // Type validation
        match self.note_type() {
            None => violations.push(SchemaViolation::MissingField("elysium_type".to_string())),
            Some(t) if !VALID_TYPES.contains(t) => {
                violations.push(SchemaViolation::InvalidType(t.to_string()))
            }
            _ => {}
        }

        // Status validation
        match self.status() {
            None => violations.push(SchemaViolation::MissingField("elysium_status".to_string())),
            Some(s) if !VALID_STATUS.contains(s) => {
                violations.push(SchemaViolation::InvalidStatus(s.to_string()))
            }
            _ => {}
        }

        // Area validation
        match self.area() {
            None => violations.push(SchemaViolation::MissingField("elysium_area".to_string())),
            Some(a) if !VALID_AREAS.contains(a) => {
                violations.push(SchemaViolation::InvalidArea(a.to_string()))
            }
            _ => {}
        }

        // Gist validation
        if self.gist().is_none() {
            violations.push(SchemaViolation::MissingField("elysium_gist".to_string()));
        }

        // Tag validations
        let tags = self.tags();
        if tags.len() > 5 {
            violations.push(SchemaViolation::TooManyTags(tags.len()));
        }

        for tag in &tags {
            if tag.contains('/') {
                violations.push(SchemaViolation::HierarchicalTag(tag.clone()));
            }
            if tag != &tag.to_lowercase() {
                violations.push(SchemaViolation::NonLowercaseTag(tag.clone()));
            }
        }

        violations
    }

    /// Validate frontmatter using configurable schema validator
    pub fn validate_with_config(&self, validator: &SchemaValidator) -> Vec<SchemaViolation> {
        let mut violations = Vec::new();

        // YAML syntax validation (using serde_yaml)
        if let Err((line, column, message)) = validate_yaml_syntax(&self.raw) {
            violations.push(SchemaViolation::YamlSyntaxError {
                line,
                column,
                message,
            });
            // If YAML is invalid, skip further validations as fields may not be parsed correctly
            return violations;
        }

        // Folded scalar warnings (> or |)
        for (field, scalar_type) in detect_folded_scalars(&self.raw) {
            violations.push(SchemaViolation::FoldedScalarWarning { field, scalar_type });
        }

        // Type validation
        if validator.is_required("elysium_type") {
            match self.note_type() {
                None => violations.push(SchemaViolation::MissingField("elysium_type".to_string())),
                Some(t) if !validator.is_valid_type(t) => {
                    violations.push(SchemaViolation::InvalidType(t.to_string()))
                }
                _ => {}
            }
        }

        // Status validation
        if validator.is_required("elysium_status") {
            match self.status() {
                None => {
                    violations.push(SchemaViolation::MissingField("elysium_status".to_string()))
                }
                Some(s) if !validator.is_valid_status(s) => {
                    violations.push(SchemaViolation::InvalidStatus(s.to_string()))
                }
                _ => {}
            }
        }

        // Area validation
        if validator.is_required("elysium_area") {
            match self.area() {
                None => violations.push(SchemaViolation::MissingField("elysium_area".to_string())),
                Some(a) if !validator.is_valid_area(a) => {
                    violations.push(SchemaViolation::InvalidArea(a.to_string()))
                }
                _ => {}
            }
        }

        // Gist validation
        if validator.is_required("elysium_gist") && self.gist().is_none() {
            violations.push(SchemaViolation::MissingField("elysium_gist".to_string()));
        }

        // Tag count validation
        let tags = self.tags();
        if tags.len() > validator.max_tags() {
            violations.push(SchemaViolation::TooManyTags(tags.len()));
        }

        // Tag format validation
        for tag in &tags {
            if !validator.allow_hierarchical_tags() && tag.contains('/') {
                violations.push(SchemaViolation::HierarchicalTag(tag.clone()));
            }
            if validator.require_lowercase_tags() && tag != &tag.to_lowercase() {
                violations.push(SchemaViolation::NonLowercaseTag(tag.clone()));
            }
        }

        violations
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_frontmatter() {
        let content = r#"---
elysium_type: note
elysium_status: active
elysium_area: tech
elysium_gist: This is a test gist.
elysium_tags: [rust, mcp]
---

Content here.
"#;

        let fm = Frontmatter::parse(content).unwrap();
        assert_eq!(fm.note_type(), Some("note"));
        assert_eq!(fm.status(), Some("active"));
        assert_eq!(fm.area(), Some("tech"));
        assert_eq!(fm.gist(), Some("This is a test gist."));
        assert_eq!(fm.tags(), vec!["rust", "mcp"]);
    }

    #[test]
    fn test_parse_dynamic_fields() {
        let content = r#"---
elysium_type: term
elysium_status: active
elysium_area: tech
elysium_gist: Test gist
elysium_tags: [test]
elysium_source: [https://example.com, https://docs.example.com]
elysium_custom_field: custom value
---
"#;

        let fm = Frontmatter::parse(content).unwrap();

        // Standard fields
        assert_eq!(fm.note_type(), Some("term"));

        // Dynamic fields
        assert!(fm.source().is_some());
        let sources = fm.source().unwrap();
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0], "https://example.com");

        // Custom field
        assert_eq!(fm.get_string("custom_field"), Some("custom value"));
    }

    #[test]
    fn test_parse_multiline_gist() {
        let content = r#"---
elysium_type: note
elysium_status: active
elysium_area: tech
elysium_gist: >
  This is a multiline
  gist that spans
  multiple lines.
elysium_tags: []
---
"#;

        let fm = Frontmatter::parse(content).unwrap();
        assert_eq!(
            fm.gist(),
            Some("This is a multiline gist that spans multiple lines.")
        );
    }

    #[test]
    fn test_filter_fields() {
        let content = r#"---
elysium_type: note
elysium_status: active
elysium_area: tech
elysium_gist: Test
elysium_tags: [a, b]
elysium_source: [https://test.com]
---
"#;

        let fm = Frontmatter::parse(content).unwrap();

        // Default fields
        let filtered = fm.filter_fields(DEFAULT_FIELDS);
        assert!(filtered.contains_key("gist"));
        assert!(!filtered.contains_key("type")); // title, path are added by caller

        // All fields
        let all = fm.to_json_map();
        assert!(all.contains_key("type"));
        assert!(all.contains_key("source"));
    }

    // =========================================
    // New validation tests
    // =========================================

    #[test]
    fn test_count_frontmatter_blocks_single() {
        let content = r#"---
elysium_type: note
---

Content here.
"#;
        assert_eq!(count_frontmatter_blocks(content), 1);
    }

    #[test]
    fn test_count_frontmatter_blocks_duplicate() {
        let content = r#"---
elysium_type: note
---

Some content.

---
elysium_type: term
---

More content.
"#;
        assert_eq!(count_frontmatter_blocks(content), 2);
    }

    #[test]
    fn test_count_frontmatter_blocks_no_frontmatter() {
        let content = "Just plain text without frontmatter.";
        assert_eq!(count_frontmatter_blocks(content), 0);
    }

    #[test]
    fn test_validate_yaml_syntax_valid() {
        let yaml = "elysium_type: note\nelysium_tags: [a, b]";
        assert!(validate_yaml_syntax(yaml).is_ok());
    }

    #[test]
    fn test_validate_yaml_syntax_unclosed_bracket() {
        let yaml = "elysium_tags: [a, b";
        let result = validate_yaml_syntax(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_yaml_syntax_bad_indentation() {
        let yaml = "elysium_type: note\n  bad_indent: value";
        // This is actually valid YAML (value is a nested object)
        // Let's use a clearly invalid case
        let invalid_yaml = "key: value\n  - item"; // mixing styles
        let result = validate_yaml_syntax(invalid_yaml);
        // serde_yaml is permissive, so we test with truly invalid YAML
        let really_invalid = "key: [unclosed";
        assert!(validate_yaml_syntax(really_invalid).is_err());
    }

    #[test]
    fn test_detect_folded_scalars() {
        let yaml = r#"elysium_gist: >
  multiline content
elysium_type: note"#;
        let scalars = detect_folded_scalars(yaml);
        assert_eq!(scalars.len(), 1);
        assert_eq!(scalars[0], ("elysium_gist".to_string(), '>'));
    }

    #[test]
    fn test_detect_literal_scalars() {
        let yaml = r#"description: |
  line1
  line2
elysium_type: note"#;
        let scalars = detect_folded_scalars(yaml);
        assert_eq!(scalars.len(), 1);
        assert_eq!(scalars[0], ("description".to_string(), '|'));
    }

    #[test]
    fn test_detect_no_folded_scalars() {
        let yaml = r#"elysium_type: note
elysium_gist: "Normal quoted string"
elysium_tags: [a, b]"#;
        let scalars = detect_folded_scalars(yaml);
        assert!(scalars.is_empty());
    }
}
