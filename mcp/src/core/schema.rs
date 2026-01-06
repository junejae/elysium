use std::collections::HashSet;

use lazy_static::lazy_static;

lazy_static! {
    pub static ref VALID_TYPES: HashSet<&'static str> =
        HashSet::from(["note", "term", "project", "log"]);
    pub static ref VALID_STATUS: HashSet<&'static str> =
        HashSet::from(["active", "done", "archived"]);
    pub static ref VALID_AREAS: HashSet<&'static str> =
        HashSet::from(["work", "tech", "life", "career", "learning", "reference"]);
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

impl std::fmt::Display for SchemaViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingFrontmatter => write!(f, "Missing YAML frontmatter"),
            Self::MissingField(field) => write!(f, "Missing required field: {}", field),
            Self::InvalidType(t) => {
                write!(f, "Invalid type '{}' (must be: note|term|project|log)", t)
            }
            Self::InvalidStatus(s) => {
                write!(f, "Invalid status '{}' (must be: active|done|archived)", s)
            }
            Self::InvalidArea(a) => write!(
                f,
                "Invalid area '{}' (must be: work|tech|life|career|learning|reference)",
                a
            ),
            Self::TooManyTags(n) => write!(f, "Too many tags: {} (max 5)", n),
            Self::HierarchicalTag(t) => write!(f, "Hierarchical tag not allowed: {}", t),
            Self::NonLowercaseTag(t) => write!(f, "Tag must be lowercase: {}", t),
            Self::EmptyGist => write!(f, "Gist field is empty"),
        }
    }
}
