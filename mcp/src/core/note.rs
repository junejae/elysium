use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::{DateTime, Local};

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

    pub fn folder(&self) -> &str {
        self.path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .unwrap_or("")
    }

    pub fn validate_schema(&self) -> Vec<SchemaViolation> {
        match &self.frontmatter {
            Some(fm) => fm.validate(),
            None => vec![SchemaViolation::MissingFrontmatter],
        }
    }

    /// Validate schema using configurable validator
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
            .map(|fm| fm.tags.clone())
            .unwrap_or_default()
    }

    pub fn note_type(&self) -> Option<&str> {
        self.frontmatter.as_ref()?.note_type.as_deref()
    }

    pub fn status(&self) -> Option<&str> {
        self.frontmatter.as_ref()?.status.as_deref()
    }

    pub fn area(&self) -> Option<&str> {
        self.frontmatter.as_ref()?.area.as_deref()
    }

    pub fn gist(&self) -> Option<&str> {
        self.frontmatter.as_ref()?.gist.as_deref()
    }

    pub fn check_folder_type_match(&self) -> bool {
        let folder = self.folder();
        let note_type = self.note_type();
        let status = self.status();

        match (note_type, status) {
            (Some("project"), Some("archived")) => folder == "Archive",
            (Some("project"), _) => folder == "Projects",
            (Some("note") | Some("term") | Some("log"), _) => folder == "Notes",
            _ => true,
        }
    }
}

pub fn collect_all_notes(paths: &VaultPaths) -> Vec<Note> {
    let mut notes = Vec::new();

    for dir in paths.content_dirs() {
        if !dir.exists() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "md").unwrap_or(false) {
                    if let Ok(note) = Note::load(&path) {
                        notes.push(note);
                    }
                }
            }
        }
    }

    notes.sort_by(|a, b| a.name.cmp(&b.name));
    notes
}

pub fn collect_note_names(paths: &VaultPaths) -> HashSet<String> {
    let mut names = HashSet::new();

    for dir in paths.content_dirs() {
        if !dir.exists() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "md").unwrap_or(false) {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        names.insert(stem.to_string());
                    }
                }
            }
        }
    }

    names
}
