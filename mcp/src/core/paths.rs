//! Vault path management

use std::path::PathBuf;

use super::config::Config;

pub const VAULT_PATH_ENV: &str = "ELYSIUM_VAULT_PATH";

pub struct VaultPaths {
    pub root: PathBuf,
    #[allow(dead_code)]
    pub inbox: PathBuf,
    pub config: Config,
}

impl VaultPaths {
    pub fn new() -> Self {
        let root = get_vault_root();
        Self::from_root(root)
    }

    pub fn from_root(root: PathBuf) -> Self {
        let config = Config::load(&root);
        Self::from_root_with_config(root, config)
    }

    pub fn from_root_with_config(root: PathBuf, config: Config) -> Self {
        let resolved = config.resolve_paths(&root);

        Self {
            inbox: resolved.inbox,
            root,
            config,
        }
    }

    #[allow(dead_code)]
    pub fn get_config(&self) -> &Config {
        &self.config
    }
}

impl Default for VaultPaths {
    fn default() -> Self {
        Self::new()
    }
}

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
