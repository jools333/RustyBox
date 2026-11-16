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

/// How DNS requests from applications are handled.
///
/// `Vpn` sends every DNS query through the tunnel (the old behaviour).
/// `Auto` keeps private/LAN DNS servers (for example a router or AdGuard at
/// 192.168.1.36) outside the tunnel, while public DNS still goes through the
/// VPN.  This keeps name resolution alive when the tunnel is unhealthy.
/// `Direct` bypasses the tunnel for all DNS queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DnsMode {
    Vpn,
    #[default]
    Auto,
    Direct,
}

impl DnsMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            DnsMode::Vpn => "vpn",
            DnsMode::Auto => "auto",
            DnsMode::Direct => "direct",
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_tun_mtu() -> u32 {
    1500
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    pub version: u32,
    pub active_profile_id: Option<String>,
    #[serde(default)]
    pub work_profile_id: Option<String>,
    #[serde(default)]
    pub work_profile_enabled: bool,
    pub routing_mode: RoutingMode,
    pub default_route_action: RuleAction,
    pub rules: Vec<RoutingRule>,
    pub profiles: Vec<Profile>,
    pub remote_dns: String,
    pub direct_dns: String,
    #[serde(default)]
    pub dns_mode: DnsMode,
    #[serde(default = "default_true")]
    pub tun_ipv6: bool,
    #[serde(default = "default_true")]
    pub health_check: bool,
    pub close_to_tray: bool,
    pub singbox_path: Option<String>,
    pub tun_interface_name: String,
    #[serde(default = "default_tun_mtu")]
    pub tun_mtu: u32,
    pub log_level: String,
    #[serde(default = "default_true")]
    pub auto_check_updates: bool,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default)]
    pub auto_connect: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: 1,
            active_profile_id: None,
            work_profile_id: None,
            work_profile_enabled: false,
            routing_mode: RoutingMode::DomainRules,
            default_route_action: RuleAction::Vpn,
            rules: vec![
                RoutingRule::new("*.local", RuleAction::Direct),
                RoutingRule::new("localhost", RuleAction::Direct),
            ],
            profiles: vec![],
            remote_dns: "1.1.1.1".to_string(),
            direct_dns: "77.88.8.8".to_string(),
            dns_mode: DnsMode::Auto,
            tun_ipv6: true,
            health_check: true,
            close_to_tray: true,
            singbox_path: None,
            tun_interface_name: if cfg!(target_os = "macos") {
                String::new()
            } else {
                "rustybox-tun".to_string()
            },
            tun_mtu: 1500,
            log_level: "info".to_string(),
            auto_check_updates: true,
            auto_start: false,
            auto_connect: false,
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

    pub fn get(&self) -> AppConfig {
        self.get_config()
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
        cfg.auto_connect = true;
        cfg.remote_dns = "8.8.8.8".to_string();
        cfg.auto_start = true;
        cfg.tun_mtu = 1420;
        cfg.dns_mode = DnsMode::Direct;
        cfg.tun_ipv6 = false;
        cfg.health_check = false;
        mgr.save_config(cfg.clone()).unwrap();

        let mgr2 = ConfigManager::new(path);
        let loaded = mgr2.get_config();
        assert!(!loaded.close_to_tray);
        assert!(loaded.auto_connect);
        assert_eq!(loaded.remote_dns, "8.8.8.8");
        assert!(loaded.auto_start);
        assert_eq!(loaded.tun_mtu, 1420);
        assert_eq!(loaded.dns_mode, DnsMode::Direct);
        assert!(!loaded.tun_ipv6);
        assert!(!loaded.health_check);
    }

    #[test]
    fn test_old_config_gets_safe_dns_defaults() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        // An old v1 config without the DNS/IPv6/health fields must still load.
        fs::write(
            &path,
            r#"{
                "version": 1,
                "active_profile_id": null,
                "routing_mode": "domain_rules",
                "default_route_action": "VPN",
                "rules": [],
                "profiles": [],
                "remote_dns": "1.1.1.1",
                "direct_dns": "77.88.8.8",
                "close_to_tray": true,
                "singbox_path": null,
                "tun_interface_name": "rustybox-tun",
                "log_level": "info"
            }"#,
        )
        .unwrap();

        let cfg = ConfigManager::new(path).get_config();
        assert_eq!(cfg.dns_mode, DnsMode::Auto);
        assert!(cfg.tun_ipv6);
        assert!(cfg.health_check);
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
