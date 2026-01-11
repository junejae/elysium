//! Elysium configuration module
//!
//! Config loading priority:
//! 1. Plugin config: .obsidian/plugins/elysium/config.json (SSOT)
//! 2. Legacy fallback: .elysium.json (for backward compatibility)
//!
//! Philosophy: MCP is a helper tool for the Obsidian plugin.
//! The plugin owns the configuration, MCP follows it.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Plugin config path (Single Source of Truth)
pub const PLUGIN_CONFIG_PATH: &str = ".obsidian/plugins/elysium/config.json";
/// Legacy config path (backward compatibility)
pub const LEGACY_CONFIG_FILE: &str = ".elysium.json";
pub const CONFIG_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_version")]
    pub version: u32,

    #[serde(default)]
    pub schema: SchemaConfig,

    #[serde(default)]
    pub folders: FoldersConfig,

    #[serde(default)]
    pub features: FeatureConfig,
}

fn default_version() -> u32 {
    CONFIG_VERSION
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
        "elysium_type".to_string(),
        "elysium_status".to_string(),
        "elysium_area".to_string(),
        "elysium_gist".to_string(),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoldersConfig {
    #[serde(default = "default_notes_folder")]
    pub notes: String,

    #[serde(default = "default_projects_folder")]
    pub projects: String,

    #[serde(default = "default_archive_folder")]
    pub archive: String,
}

fn default_notes_folder() -> String {
    "Notes".to_string()
}

fn default_projects_folder() -> String {
    "Projects".to_string()
}

fn default_archive_folder() -> String {
    "Archive".to_string()
}

impl Default for FoldersConfig {
    fn default() -> Self {
        Self {
            notes: default_notes_folder(),
            projects: default_projects_folder(),
            archive: default_archive_folder(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureConfig {
    #[serde(default = "default_inbox")]
    pub inbox: String,

    #[serde(default = "default_true")]
    pub wikilinks: bool,

    #[serde(default, rename = "semanticSearch")]
    pub semantic_search: bool,

    #[serde(default, rename = "wikilinkValidation")]
    pub wikilink_validation: bool,

    #[serde(default, rename = "advancedSemanticSearch")]
    pub advanced_semantic_search: AdvancedSemanticSearchConfig,
}

/// Default Model2Vec model ID
pub const DEFAULT_MODEL2VEC_MODEL: &str = "minishlab/potion-multilingual-128M";

/// Configuration for advanced semantic search (Model2Vec)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedSemanticSearchConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default, rename = "modelDownloaded")]
    pub model_downloaded: bool,

    #[serde(default, rename = "modelPath")]
    pub model_path: Option<String>,

    #[serde(default = "default_model_id", rename = "modelId")]
    pub model_id: String,
}

fn default_model_id() -> String {
    DEFAULT_MODEL2VEC_MODEL.to_string()
}

impl Default for AdvancedSemanticSearchConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            model_downloaded: false,
            model_path: None,
            model_id: default_model_id(),
        }
    }
}

fn default_inbox() -> String {
    "inbox.md".to_string()
}

impl Default for FeatureConfig {
    fn default() -> Self {
        Self {
            inbox: default_inbox(),
            wikilinks: true,
            semantic_search: true,
            wikilink_validation: true,
            advanced_semantic_search: AdvancedSemanticSearchConfig::default(),
        }
    }
}

impl FeatureConfig {
    /// Check if advanced semantic search is enabled and model is available
    pub fn is_advanced_search_ready(&self) -> bool {
        self.advanced_semantic_search.enabled && self.advanced_semantic_search.model_downloaded
    }

    /// Get model path for advanced search
    pub fn get_model_path(&self) -> Option<&str> {
        if self.is_advanced_search_ready() {
            self.advanced_semantic_search.model_path.as_deref()
        } else {
            None
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            schema: SchemaConfig::default(),
            folders: FoldersConfig::default(),
            features: FeatureConfig::default(),
        }
    }
}

impl Config {
    pub fn load(vault_root: &Path) -> Self {
        let plugin_config_path = vault_root.join(PLUGIN_CONFIG_PATH);
        let legacy_config_path = vault_root.join(LEGACY_CONFIG_FILE);

        if plugin_config_path.exists() {
            match Self::load_from_file(&plugin_config_path) {
                Ok(config) => {
                    if config.version > CONFIG_VERSION {
                        eprintln!(
                            "Warning: Config version {} is newer than supported version {}.",
                            config.version, CONFIG_VERSION
                        );
                    }
                    return config;
                }
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to load plugin config: {}. Trying legacy path.",
                        e
                    );
                }
            }
        }

        if legacy_config_path.exists() {
            match Self::load_from_file(&legacy_config_path) {
                Ok(config) => {
                    eprintln!(
                        "Note: Using legacy config {}. Consider migrating to plugin config.",
                        LEGACY_CONFIG_FILE
                    );
                    return config;
                }
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to load {}: {}. Using defaults.",
                        LEGACY_CONFIG_FILE, e
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

    pub fn save(&self, vault_root: &Path) -> Result<()> {
        let config_path = vault_root.join(PLUGIN_CONFIG_PATH);
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(config_path, content)?;
        Ok(())
    }

    /// Generate default config file content
    pub fn default_json() -> String {
        serde_json::to_string_pretty(&Config::default()).unwrap()
    }

    /// Get resolved paths based on vault root
    pub fn resolve_paths(&self, vault_root: &Path) -> ResolvedPaths {
        ResolvedPaths {
            root: vault_root.to_path_buf(),
            inbox: vault_root.join(&self.features.inbox),
        }
    }
}

/// Resolved absolute paths for vault
#[derive(Debug, Clone)]
pub struct ResolvedPaths {
    pub root: PathBuf,
    pub inbox: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.version, 1);
        assert_eq!(config.schema.types.len(), 4);
        assert!(config.schema.lowercase_tags);
    }

    #[test]
    fn test_parse_partial_config() {
        let json = r#"{"features": {"inbox": "my-inbox.md"}}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.features.inbox, "my-inbox.md");
        assert!(config.features.wikilinks);
    }

    #[test]
    fn test_schema_sets() {
        let config = Config::default();
        let types = config.schema.types_set();
        assert!(types.contains("note"));
        assert!(types.contains("term"));
    }
}
