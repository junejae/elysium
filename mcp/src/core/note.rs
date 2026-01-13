use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::{DateTime, Local};
use walkdir::WalkDir;

use super::frontmatter::Frontmatter;
use super::paths::VaultPaths;
use super::schema::{SchemaValidator, SchemaViolation};
use super::wikilink::extract_wikilinks;

pub struct Note {
    pub path: PathBuf,
    pub name: String,
    pub content: String,
    pub frontmatter: Option<Frontmatter>,
    pub modified: DateTime<Local>,
    pub created: DateTime<Local>,
}

impl Note {
    pub fn load(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let metadata = fs::metadata(path)?;

        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        let frontmatter = Frontmatter::parse(&content);
        let modified = DateTime::from(metadata.modified()?);
        let created = DateTime::from(metadata.created().unwrap_or(metadata.modified()?));

        Ok(Self {
            path: path.to_path_buf(),
            name,
            content,
            frontmatter,
            modified,
            created,
        })
    }

    pub fn validate_schema(&self) -> Vec<SchemaViolation> {
        match &self.frontmatter {
            Some(fm) => fm.validate(),
            None => vec![SchemaViolation::MissingFrontmatter],
        }
    }

    pub fn validate_schema_with_config(&self, validator: &SchemaValidator) -> Vec<SchemaViolation> {
        match &self.frontmatter {
            Some(fm) => fm.validate_with_config(validator),
            None => vec![SchemaViolation::MissingFrontmatter],
        }
    }

    pub fn wikilinks(&self) -> Vec<String> {
        extract_wikilinks(&self.content)
    }

    pub fn tags(&self) -> Vec<String> {
        self.frontmatter
            .as_ref()
            .map(|fm| fm.tags())
            .unwrap_or_default()
    }

    pub fn note_type(&self) -> Option<&str> {
        self.frontmatter.as_ref()?.note_type()
    }

    pub fn status(&self) -> Option<&str> {
        self.frontmatter.as_ref()?.status()
    }

    pub fn area(&self) -> Option<&str> {
        self.frontmatter.as_ref()?.area()
    }

    pub fn gist(&self) -> Option<&str> {
        self.frontmatter.as_ref()?.gist()
    }

    /// Get source URLs (elysium_source)
    pub fn source(&self) -> Option<Vec<String>> {
        self.frontmatter.as_ref()?.source()
    }

    /// Get any dynamic field by key (without elysium_ prefix)
    pub fn get_field(&self, key: &str) -> Option<&super::frontmatter::FieldValue> {
        self.frontmatter.as_ref()?.get(key)
    }

    /// Get all frontmatter fields as JSON map
    pub fn fields_to_json(&self) -> std::collections::HashMap<String, serde_json::Value> {
        self.frontmatter
            .as_ref()
            .map(|fm| fm.to_json_map())
            .unwrap_or_default()
    }
}

fn should_exclude_path(path: &Path) -> bool {
    path.components().any(|c| {
        c.as_os_str()
            .to_str()
            .map(|s| s.starts_with('.'))
            .unwrap_or(false)
    })
}

pub fn collect_all_notes(paths: &VaultPaths) -> Vec<Note> {
    let mut notes = Vec::new();

    for entry in WalkDir::new(&paths.root).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();

        if should_exclude_path(path) {
            continue;
        }

        if path.extension().map(|e| e == "md").unwrap_or(false) {
            if let Ok(note) = Note::load(path) {
                notes.push(note);
            }
        }
    }

    notes.sort_by(|a, b| a.name.cmp(&b.name));
    notes
}

pub fn collect_note_names(paths: &VaultPaths) -> HashSet<String> {
    let mut names = HashSet::new();

    for entry in WalkDir::new(&paths.root).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();

        if should_exclude_path(path) {
            continue;
        }

        if path.extension().map(|e| e == "md").unwrap_or(false) {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                names.insert(stem.to_string());
            }
        }
    }

    names
}
