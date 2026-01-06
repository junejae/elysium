//! Elysium configuration module
//!
//! Loads configuration from .elysium.json in vault root with sensible defaults.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub const CONFIG_FILE_NAME: &str = ".elysium.json";
pub const CONFIG_VERSION: u32 = 1;

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_version")]
    pub version: u32,

    #[serde(default)]
    pub folders: FolderConfig,

    #[serde(default)]
    pub schema: SchemaConfig,

    #[serde(default)]
    pub features: FeatureConfig,
}

fn default_version() -> u32 {
    CONFIG_VERSION
}

/// Folder structure configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderConfig {
    #[serde(default = "default_notes")]
    pub notes: String,

    #[serde(default = "default_projects")]
    pub projects: String,

    #[serde(default = "default_archive")]
    pub archive: String,

    #[serde(default = "default_system")]
    pub system: String,

    #[serde(default = "default_templates")]
    pub templates: String,

    #[serde(default = "default_attachments")]
    pub attachments: String,
}

fn default_notes() -> String {
    "Notes".to_string()
}
fn default_projects() -> String {
    "Projects".to_string()
}
fn default_archive() -> String {
    "Archive".to_string()
}
fn default_system() -> String {
    "_system".to_string()
}
fn default_templates() -> String {
    "_system/Templates".to_string()
}
fn default_attachments() -> String {
    "_system/Attachments".to_string()
}

impl Default for FolderConfig {
    fn default() -> Self {
        Self {
            notes: default_notes(),
            projects: default_projects(),
            archive: default_archive(),
            system: default_system(),
            templates: default_templates(),
            attachments: default_attachments(),
        }
    }
}

/// Schema validation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaConfig {
    #[serde(default = "default_types")]
    pub types: Vec<String>,

    #[serde(default = "default_statuses")]
    pub statuses: Vec<String>,

    #[serde(default = "default_areas")]
    pub areas: Vec<String>,

    #[serde(default = "default_required_fields")]
    pub required_fields: Vec<String>,

    #[serde(default = "default_max_tags")]
    pub max_tags: usize,

    #[serde(default = "default_true")]
    pub lowercase_tags: bool,

    #[serde(default)]
    pub allow_hierarchical_tags: bool,
}

fn default_types() -> Vec<String> {
    vec![
        "note".to_string(),
        "term".to_string(),
        "project".to_string(),
        "log".to_string(),
    ]
}

fn default_statuses() -> Vec<String> {
    vec![
        "active".to_string(),
        "done".to_string(),
        "archived".to_string(),
    ]
}

fn default_areas() -> Vec<String> {
    vec![
        "work".to_string(),
        "tech".to_string(),
        "life".to_string(),
        "career".to_string(),
        "learning".to_string(),
        "reference".to_string(),
    ]
}

fn default_required_fields() -> Vec<String> {
    vec![
        "type".to_string(),
        "status".to_string(),
        "area".to_string(),
        "gist".to_string(),
    ]
}

fn default_max_tags() -> usize {
    5
}

fn default_true() -> bool {
    true
}

impl Default for SchemaConfig {
    fn default() -> Self {
        Self {
            types: default_types(),
            statuses: default_statuses(),
            areas: default_areas(),
            required_fields: default_required_fields(),
            max_tags: default_max_tags(),
            lowercase_tags: true,
            allow_hierarchical_tags: false,
        }
    }
}

impl SchemaConfig {
    pub fn types_set(&self) -> HashSet<String> {
        self.types.iter().cloned().collect()
    }

    pub fn statuses_set(&self) -> HashSet<String> {
        self.statuses.iter().cloned().collect()
    }

    pub fn areas_set(&self) -> HashSet<String> {
        self.areas.iter().cloned().collect()
    }

    pub fn is_required(&self, field: &str) -> bool {
        self.required_fields.iter().any(|f| f == field)
    }
}

/// Feature toggles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureConfig {
    #[serde(default = "default_inbox")]
    pub inbox: String,

    #[serde(default = "default_true")]
    pub wikilinks: bool,
}

fn default_inbox() -> String {
    "inbox.md".to_string()
}

impl Default for FeatureConfig {
    fn default() -> Self {
        Self {
            inbox: default_inbox(),
            wikilinks: true,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            folders: FolderConfig::default(),
            schema: SchemaConfig::default(),
            features: FeatureConfig::default(),
        }
    }
}

impl Config {
    /// Load configuration from vault root, or return defaults if not found
    pub fn load(vault_root: &Path) -> Self {
        let config_path = vault_root.join(CONFIG_FILE_NAME);

        if config_path.exists() {
            match Self::load_from_file(&config_path) {
                Ok(config) => {
                    if config.version > CONFIG_VERSION {
                        eprintln!(
                            "Warning: Config version {} is newer than supported version {}. Some features may not work.",
                            config.version, CONFIG_VERSION
                        );
                    }
                    return config;
                }
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to load {}: {}. Using defaults.",
                        CONFIG_FILE_NAME, e
                    );
                }
            }
        }

        Self::default()
    }

    fn load_from_file(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Config = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to file
    pub fn save(&self, vault_root: &Path) -> Result<()> {
        let config_path = vault_root.join(CONFIG_FILE_NAME);
        let content = serde_json::to_string_pretty(self)?;
        fs::write(config_path, content)?;
        Ok(())
    }

    /// Generate default config file content
    pub fn default_json() -> String {
        serde_json::to_string_pretty(&Config::default()).unwrap()
    }

    /// Get resolved folder paths based on vault root
    pub fn resolve_paths(&self, vault_root: &Path) -> ResolvedPaths {
        ResolvedPaths {
            root: vault_root.to_path_buf(),
            notes: vault_root.join(&self.folders.notes),
            projects: vault_root.join(&self.folders.projects),
            archive: vault_root.join(&self.folders.archive),
            system: vault_root.join(&self.folders.system),
            templates: vault_root.join(&self.folders.templates),
            attachments: vault_root.join(&self.folders.attachments),
            inbox: vault_root.join(&self.features.inbox),
        }
    }
}

/// Resolved absolute paths for vault folders
#[derive(Debug, Clone)]
pub struct ResolvedPaths {
    pub root: PathBuf,
    pub notes: PathBuf,
    pub projects: PathBuf,
    pub archive: PathBuf,
    pub system: PathBuf,
    pub templates: PathBuf,
    pub attachments: PathBuf,
    pub inbox: PathBuf,
}

impl ResolvedPaths {
    /// Get content directories (notes, projects, archive)
    pub fn content_dirs(&self) -> Vec<&PathBuf> {
        vec![&self.notes, &self.projects, &self.archive]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.version, 1);
        assert_eq!(config.folders.notes, "Notes");
        assert_eq!(config.schema.types.len(), 4);
        assert!(config.schema.lowercase_tags);
    }

    #[test]
    fn test_parse_partial_config() {
        let json = r#"{"folders": {"notes": "Zettelkasten"}}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.folders.notes, "Zettelkasten");
        assert_eq!(config.folders.projects, "Projects"); // default
    }

    #[test]
    fn test_schema_sets() {
        let config = Config::default();
        let types = config.schema.types_set();
        assert!(types.contains("note"));
        assert!(types.contains("term"));
    }
}
