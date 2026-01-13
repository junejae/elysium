//! Schema validation for vault notes
//!
//! Validates frontmatter against configurable schema rules.

use std::collections::HashSet;

use super::config::SchemaConfig;

/// Legacy static sets for backward compatibility
/// These are used when no config is available
pub fn default_types() -> HashSet<&'static str> {
    HashSet::from(["note", "term", "project", "log", "lesson"])
}

pub fn default_statuses() -> HashSet<&'static str> {
    HashSet::from(["active", "done", "archived"])
}

pub fn default_areas() -> HashSet<&'static str> {
    HashSet::from([
        "work",
        "tech",
        "life",
        "career",
        "learning",
        "reference",
        "defense",
        "prosecutor",
        "judge",
    ])
}

// Keep these for backward compatibility with existing code
lazy_static::lazy_static! {
    pub static ref VALID_TYPES: HashSet<&'static str> = default_types();
    pub static ref VALID_STATUS: HashSet<&'static str> = default_statuses();
    pub static ref VALID_AREAS: HashSet<&'static str> = default_areas();
}

#[derive(Debug, Clone, PartialEq)]
pub enum SchemaViolation {
    MissingFrontmatter,
    MissingField(String),
    InvalidType(String),
    InvalidStatus(String),
    InvalidArea(String),
    TooManyTags(usize),
    HierarchicalTag(String),
    NonLowercaseTag(String),
    EmptyGist,
}

impl SchemaViolation {
    /// Format violation message with config-aware valid values
    pub fn format_with_config(&self, config: &SchemaConfig) -> String {
        match self {
            Self::MissingFrontmatter => "Missing YAML frontmatter".to_string(),
            Self::MissingField(field) => format!("Missing required field: {}", field),
            Self::InvalidType(t) => {
                format!("Invalid type '{}' (must be: {})", t, config.types.join("|"))
            }
            Self::InvalidStatus(s) => {
                format!(
                    "Invalid status '{}' (must be: {})",
                    s,
                    config.statuses.join("|")
                )
            }
            Self::InvalidArea(a) => {
                format!("Invalid area '{}' (must be: {})", a, config.areas.join("|"))
            }
            Self::TooManyTags(n) => format!("Too many tags: {} (max {})", n, config.max_tags),
            Self::HierarchicalTag(t) => format!("Hierarchical tag not allowed: {}", t),
            Self::NonLowercaseTag(t) => format!("Tag must be lowercase: {}", t),
            Self::EmptyGist => "Gist field is empty".to_string(),
        }
    }
}

impl std::fmt::Display for SchemaViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Default display uses hardcoded values for backward compatibility
        match self {
            Self::MissingFrontmatter => write!(f, "Missing YAML frontmatter"),
            Self::MissingField(field) => write!(f, "Missing required field: {}", field),
            Self::InvalidType(t) => {
                write!(
                    f,
                    "Invalid elysium_type '{}' (must be: note|term|project|log|lesson)",
                    t
                )
            }
            Self::InvalidStatus(s) => {
                write!(
                    f,
                    "Invalid elysium_status '{}' (must be: active|done|archived)",
                    s
                )
            }
            Self::InvalidArea(a) => write!(
                f,
                "Invalid elysium_area '{}' (must be: work|tech|life|career|learning|reference|defense|prosecutor|judge)",
                a
            ),
            Self::TooManyTags(n) => write!(f, "Too many elysium_tags: {} (max 5)", n),
            Self::HierarchicalTag(t) => write!(f, "Hierarchical tag not allowed: {}", t),
            Self::NonLowercaseTag(t) => write!(f, "Tag must be lowercase: {}", t),
            Self::EmptyGist => write!(f, "elysium_gist field is empty"),
        }
    }
}

/// Schema validator with configurable rules
pub struct SchemaValidator {
    types: HashSet<String>,
    statuses: HashSet<String>,
    areas: HashSet<String>,
    required_fields: HashSet<String>,
    max_tags: usize,
    lowercase_tags: bool,
    allow_hierarchical_tags: bool,
}

impl SchemaValidator {
    /// Create validator from config
    pub fn from_config(config: &SchemaConfig) -> Self {
        Self {
            types: config.types_set(),
            statuses: config.statuses_set(),
            areas: config.areas_set(),
            required_fields: config.required_fields.iter().cloned().collect(),
            max_tags: config.max_tags,
            lowercase_tags: config.lowercase_tags,
            allow_hierarchical_tags: config.allow_hierarchical_tags,
        }
    }

    /// Create validator with default (hardcoded) values
    pub fn default() -> Self {
        Self {
            types: default_types().iter().map(|s| s.to_string()).collect(),
            statuses: default_statuses().iter().map(|s| s.to_string()).collect(),
            areas: default_areas().iter().map(|s| s.to_string()).collect(),
            required_fields: [
                "elysium_type",
                "elysium_status",
                "elysium_area",
                "elysium_gist",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            max_tags: 5,
            lowercase_tags: true,
            allow_hierarchical_tags: false,
        }
    }

    pub fn is_valid_type(&self, t: &str) -> bool {
        self.types.contains(t)
    }

    pub fn is_valid_status(&self, s: &str) -> bool {
        self.statuses.contains(s)
    }

    pub fn is_valid_area(&self, a: &str) -> bool {
        self.areas.contains(a)
    }

    pub fn is_required(&self, field: &str) -> bool {
        self.required_fields.contains(field)
    }

    pub fn max_tags(&self) -> usize {
        self.max_tags
    }

    pub fn require_lowercase_tags(&self) -> bool {
        self.lowercase_tags
    }

    pub fn allow_hierarchical_tags(&self) -> bool {
        self.allow_hierarchical_tags
    }
}
