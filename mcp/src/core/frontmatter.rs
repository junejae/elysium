//! Frontmatter parsing and validation
//!
//! Parses YAML frontmatter from markdown notes and validates against schema.

use lazy_static::lazy_static;
use regex::Regex;

use super::schema::{SchemaValidator, SchemaViolation, VALID_AREAS, VALID_STATUS, VALID_TYPES};

lazy_static! {
    static ref FRONTMATTER_RE: Regex = Regex::new(r"(?s)^---\r?\n(.*?)\r?\n---").unwrap();
    static ref TYPE_RE: Regex = Regex::new(r"(?m)^type:\s*(\w+)").unwrap();
    static ref STATUS_RE: Regex = Regex::new(r"(?m)^status:\s*(\w+)").unwrap();
    static ref AREA_RE: Regex = Regex::new(r"(?m)^area:\s*(\w+)").unwrap();
    static ref GIST_RE: Regex = Regex::new(r"(?m)^gist:\s*(.*)").unwrap();
    static ref TAGS_RE: Regex = Regex::new(r"(?m)^tags:\s*\[(.*?)\]").unwrap();
}

#[derive(Debug, Default, Clone)]
pub struct Frontmatter {
    pub note_type: Option<String>,
    pub status: Option<String>,
    pub area: Option<String>,
    pub gist: Option<String>,
    pub tags: Vec<String>,
    pub raw: String,
}

impl Frontmatter {
    pub fn parse(content: &str) -> Option<Self> {
        let caps = FRONTMATTER_RE.captures(content)?;
        let raw = caps.get(1)?.as_str().to_string();

        let note_type = TYPE_RE.captures(&raw).map(|c| c[1].to_string());
        let status = STATUS_RE.captures(&raw).map(|c| c[1].to_string());
        let area = AREA_RE.captures(&raw).map(|c| c[1].to_string());
        let gist = Self::extract_gist(&raw);
        let tags = Self::extract_tags(&raw);

        Some(Self {
            note_type,
            status,
            area,
            gist,
            tags,
            raw,
        })
    }

    fn extract_gist(raw: &str) -> Option<String> {
        if let Some(caps) = GIST_RE.captures(raw) {
            let gist_start = caps.get(1)?.as_str().trim();

            if gist_start == ">" || gist_start == "|" || gist_start.is_empty() {
                let lines: Vec<&str> = raw.lines().collect();
                let gist_line_idx = lines.iter().position(|l| l.starts_with("gist:"))?;

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
                let gist = gist_start.trim_matches('"').trim_matches('\'').to_string();
                if gist.is_empty() {
                    None
                } else {
                    Some(gist)
                }
            }
        } else {
            None
        }
    }

    fn extract_tags(raw: &str) -> Vec<String> {
        TAGS_RE
            .captures(raw)
            .map(|c| {
                c[1].split(',')
                    .map(|t| t.trim().trim_matches('"').trim_matches('\'').to_string())
                    .filter(|t| !t.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Validate frontmatter using default schema (backward compatible)
    pub fn validate(&self) -> Vec<SchemaViolation> {
        self.validate_with_defaults()
    }

    /// Validate frontmatter using hardcoded default schema
    fn validate_with_defaults(&self) -> Vec<SchemaViolation> {
        let mut violations = Vec::new();

        match &self.note_type {
            None => violations.push(SchemaViolation::MissingField("type".to_string())),
            Some(t) if !VALID_TYPES.contains(t.as_str()) => {
                violations.push(SchemaViolation::InvalidType(t.clone()))
            }
            _ => {}
        }

        match &self.status {
            None => violations.push(SchemaViolation::MissingField("status".to_string())),
            Some(s) if !VALID_STATUS.contains(s.as_str()) => {
                violations.push(SchemaViolation::InvalidStatus(s.clone()))
            }
            _ => {}
        }

        match &self.area {
            None => violations.push(SchemaViolation::MissingField("area".to_string())),
            Some(a) if !VALID_AREAS.contains(a.as_str()) => {
                violations.push(SchemaViolation::InvalidArea(a.clone()))
            }
            _ => {}
        }

        if self.gist.is_none() {
            violations.push(SchemaViolation::MissingField("gist".to_string()));
        }

        if self.tags.len() > 5 {
            violations.push(SchemaViolation::TooManyTags(self.tags.len()));
        }

        for tag in &self.tags {
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

        // Type validation
        if validator.is_required("type") {
            match &self.note_type {
                None => violations.push(SchemaViolation::MissingField("type".to_string())),
                Some(t) if !validator.is_valid_type(t) => {
                    violations.push(SchemaViolation::InvalidType(t.clone()))
                }
                _ => {}
            }
        }

        // Status validation
        if validator.is_required("status") {
            match &self.status {
                None => violations.push(SchemaViolation::MissingField("status".to_string())),
                Some(s) if !validator.is_valid_status(s) => {
                    violations.push(SchemaViolation::InvalidStatus(s.clone()))
                }
                _ => {}
            }
        }

        // Area validation
        if validator.is_required("area") {
            match &self.area {
                None => violations.push(SchemaViolation::MissingField("area".to_string())),
                Some(a) if !validator.is_valid_area(a) => {
                    violations.push(SchemaViolation::InvalidArea(a.clone()))
                }
                _ => {}
            }
        }

        // Gist validation
        if validator.is_required("gist") && self.gist.is_none() {
            violations.push(SchemaViolation::MissingField("gist".to_string()));
        }

        // Tag count validation
        if self.tags.len() > validator.max_tags() {
            violations.push(SchemaViolation::TooManyTags(self.tags.len()));
        }

        // Tag format validation
        for tag in &self.tags {
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
