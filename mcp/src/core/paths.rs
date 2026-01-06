use std::path::PathBuf;

/// Environment variable for vault path configuration
pub const VAULT_PATH_ENV: &str = "ELYSIUM_VAULT_PATH";

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
}

impl VaultPaths {
    /// Create VaultPaths from environment variable or current directory.
    /// Priority: ELYSIUM_VAULT_PATH env var > current directory
    pub fn new() -> Self {
        let root = get_vault_root();
        Self::from_root(root)
    }

    pub fn from_root(root: PathBuf) -> Self {
        Self {
            notes: root.join("Notes"),
            projects: root.join("Projects"),
            archive: root.join("Archive"),
            system: root.join("_system"),
            dashboards: root.join("_system/Dashboards"),
            templates: root.join("_system/Templates"),
            attachments: root.join("_system/Attachments"),
            opencode: root.join(".opencode"),
            inbox: root.join("inbox.md"),
            root,
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
