use crate::profiles::Profile;
use crate::routing::{RoutingRule, RuleAction};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to parse config JSON: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("Config file is corrupted. Preserved backup at: {0}")]
    Corrupted(String),
    #[error("Lock error: {0}")]
    Lock(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RoutingMode {
    #[default]
    DomainRules,
    GlobalVpn,
    GlobalDirect,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    pub version: u32,
    pub active_profile_id: Option<String>,
    pub routing_mode: RoutingMode,
    pub default_route_action: RuleAction,
    pub rules: Vec<RoutingRule>,
    pub profiles: Vec<Profile>,
    pub remote_dns: String,
    pub direct_dns: String,
    pub close_to_tray: bool,
    pub singbox_path: Option<String>,
    pub tun_interface_name: String,
    pub log_level: String,
    #[serde(default = "default_true")]
    pub auto_check_updates: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: 1,
            active_profile_id: None,
            routing_mode: RoutingMode::DomainRules,
            default_route_action: RuleAction::Vpn,
            rules: vec![
                RoutingRule::new("*.local", RuleAction::Direct),
                RoutingRule::new("localhost", RuleAction::Direct),
            ],
            profiles: vec![],
            remote_dns: "1.1.1.1".to_string(),
            direct_dns: "77.88.8.8".to_string(),
            close_to_tray: true,
            singbox_path: None,
            tun_interface_name: "rustybox-tun".to_string(),
            log_level: "info".to_string(),
            auto_check_updates: true,
        }
    }
}

#[derive(Clone)]
pub struct ConfigManager {
    config: Arc<RwLock<AppConfig>>,
    path: PathBuf,
}

impl ConfigManager {
    pub fn new(path: PathBuf) -> Self {
        let cfg = Self::load_or_default(&path);
        Self {
            config: Arc::new(RwLock::new(cfg)),
            path,
        }
    }

    pub fn default_path() -> PathBuf {
        let base_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("rustybox");
        let _ = fs::create_dir_all(&base_dir);
        base_dir.join("config.json")
    }

    pub fn get_config(&self) -> AppConfig {
        self.config.read().map(|c| c.clone()).unwrap_or_default()
    }

    pub fn save_config(&self, new_config: AppConfig) -> Result<(), ConfigError> {
        let json = serde_json::to_string_pretty(&new_config)?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.path, json)?;
        if let Ok(mut lock) = self.config.write() {
            *lock = new_config;
        }
        Ok(())
    }

    pub fn update<F>(&self, f: F) -> Result<(), ConfigError>
    where
        F: FnOnce(&mut AppConfig),
    {
        let mut cfg = self.get_config();
        f(&mut cfg);
        self.save_config(cfg)
    }

    fn load_or_default(path: &Path) -> AppConfig {
        if !path.exists() {
            return AppConfig::default();
        }

        match fs::read_to_string(path) {
            Ok(content) => match serde_json::from_str::<AppConfig>(&content) {
                Ok(mut config) => {
                    if config.version < 1 {
                        config.version = 1;
                    }
                    config
                }
                Err(err) => {
                    tracing::error!("Config file parse error: {}", err);
                    let backup_name = format!(
                        "{}.corrupt.{}",
                        path.display(),
                        chrono::Utc::now().timestamp()
                    );
                    let _ = fs::copy(path, &backup_name);
                    tracing::warn!("Saved corrupted config copy to {}", backup_name);
                    AppConfig::default()
                }
            },
            Err(err) => {
                tracing::error!("Failed to read config file {}: {}", path.display(), err);
                AppConfig::default()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_config_save_load() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");

        let mgr = ConfigManager::new(path.clone());
        let mut cfg = mgr.get_config();
        cfg.close_to_tray = false;
        cfg.remote_dns = "8.8.8.8".to_string();
        mgr.save_config(cfg.clone()).unwrap();

        let mgr2 = ConfigManager::new(path);
        let loaded = mgr2.get_config();
        assert!(!loaded.close_to_tray);
        assert_eq!(loaded.remote_dns, "8.8.8.8");
    }

    #[test]
    fn test_corrupt_config_backup() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, "{ invalid_json: ").unwrap();

        let mgr = ConfigManager::new(path.clone());
        let cfg = mgr.get_config();
        assert_eq!(cfg.version, 1);

        assert!(path.exists());
    }
}
