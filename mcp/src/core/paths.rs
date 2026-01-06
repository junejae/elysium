//! Vault path management
//!
//! Handles vault root detection and folder path resolution.

use std::path::PathBuf;

use super::config::{Config, ResolvedPaths};

/// Environment variable for vault path configuration
pub const VAULT_PATH_ENV: &str = "ELYSIUM_VAULT_PATH";

/// Vault paths wrapper that combines config and resolved paths
pub struct VaultPaths {
    pub root: PathBuf,
    pub notes: PathBuf,
    pub projects: PathBuf,
    pub archive: PathBuf,
    pub system: PathBuf,
    pub dashboards: PathBuf,
    pub templates: PathBuf,
    pub attachments: PathBuf,
    pub opencode: PathBuf,
    pub inbox: PathBuf,
    pub config: Config,
}

impl VaultPaths {
    /// Create VaultPaths from environment variable or current directory.
    /// Loads config from vault root.
    pub fn new() -> Self {
        let root = get_vault_root();
        Self::from_root(root)
    }

    /// Create VaultPaths from a specific root directory
    pub fn from_root(root: PathBuf) -> Self {
        let config = Config::load(&root);
        Self::from_root_with_config(root, config)
    }

    /// Create VaultPaths with explicit config
    pub fn from_root_with_config(root: PathBuf, config: Config) -> Self {
        let resolved = config.resolve_paths(&root);

        Self {
            notes: resolved.notes,
            projects: resolved.projects,
            archive: resolved.archive,
            system: resolved.system.clone(),
            dashboards: resolved.system.join("Dashboards"),
            templates: resolved.templates,
            attachments: resolved.attachments,
            opencode: root.join(".opencode"),
            inbox: resolved.inbox,
            root,
            config,
        }
    }

    pub fn content_dirs(&self) -> Vec<&PathBuf> {
        vec![&self.notes, &self.projects, &self.archive]
    }

    pub fn required_folders(&self) -> Vec<(&PathBuf, &str, bool)> {
        vec![
            (&self.notes, "All notes (note, term, log)", false),
            (&self.projects, "Active projects", false),
            (&self.archive, "Completed projects", false),
            (&self.system, "System files", true),
            (&self.dashboards, "Dataview queries", false),
            (&self.templates, "Note templates", false),
            (&self.attachments, "Media files", false),
            (&self.opencode, "AI agent configuration", true),
        ]
    }

    /// Get the loaded configuration
    pub fn get_config(&self) -> &Config {
        &self.config
    }
}

impl Default for VaultPaths {
    fn default() -> Self {
        Self::new()
    }
}

/// Get vault root path from environment variable or current directory.
/// Priority: ELYSIUM_VAULT_PATH env var > current directory
pub fn get_vault_root() -> PathBuf {
    if let Ok(path) = std::env::var(VAULT_PATH_ENV) {
        let vault_path = PathBuf::from(&path);
        if vault_path.exists() {
            return vault_path;
        }
        eprintln!(
            "Warning: {} is set to '{}' but path does not exist. Falling back to current directory.",
            VAULT_PATH_ENV, path
        );
    }
    std::env::current_dir().expect("Failed to get current directory")
}
