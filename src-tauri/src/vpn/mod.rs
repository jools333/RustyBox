use crate::config::ConfigManager;
use crate::core::CoreManager;
use crate::logging::LogManager;
use crate::profiles::{Profile, ProfileManager};
use crate::routing::RoutingManager;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum VpnError {
    #[error("No active profile selected")]
    NoActiveProfile,
    #[error("Invalid state transition from {0} to {1}")]
    InvalidTransition(String, String),
    #[error("Core error: {0}")]
    Core(String),
    #[error("Already connecting or connected")]
    AlreadyActive,
    #[error("Lock error: {0}")]
    Lock(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    Disconnected,
    Starting,
    Connected,
    Stopping,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStatus {
    pub state: ConnectionState,
    pub active_profile: Option<Profile>,
    pub connected_since: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
}

#[derive(Clone)]
pub struct ConnectionManager {
    state: Arc<RwLock<ConnectionState>>,
    connected_since: Arc<RwLock<Option<DateTime<Utc>>>>,
    last_error: Arc<RwLock<Option<String>>>,
    profile_manager: ProfileManager,
    routing_manager: RoutingManager,
    config_manager: ConfigManager,
    core_manager: CoreManager,
    log_manager: LogManager,
}

impl ConnectionManager {
    pub fn new(
        profile_manager: ProfileManager,
        routing_manager: RoutingManager,
        config_manager: ConfigManager,
        core_manager: CoreManager,
        log_manager: LogManager,
    ) -> Self {
        Self {
            state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            connected_since: Arc::new(RwLock::new(None)),
            last_error: Arc::new(RwLock::new(None)),
            profile_manager,
            routing_manager,
            config_manager,
            core_manager,
            log_manager,
        }
    }

    pub fn get_state(&self) -> ConnectionState {
        self.state
            .read()
            .map(|s| s.clone())
            .unwrap_or(ConnectionState::Error)
    }

    pub fn get_status(&self) -> ConnectionStatus {
        let state = self.get_state();
        let connected_since = self.connected_since.read().ok().and_then(|t| *t);
        let error_message = self.last_error.read().ok().and_then(|e| e.clone());
        let active_profile_id = self.config_manager.get_config().active_profile_id;
        let active_profile = active_profile_id.and_then(|id| self.profile_manager.get(&id));

        ConnectionStatus {
            state,
            active_profile,
            connected_since,
            error_message,
        }
    }

    pub async fn connect(&self) -> Result<(), VpnError> {
        {
            let mut state = self
                .state
                .write()
                .map_err(|e| VpnError::Lock(e.to_string()))?;
            match *state {
                ConnectionState::Disconnected | ConnectionState::Error => {
                    *state = ConnectionState::Starting;
                }
                ConnectionState::Starting | ConnectionState::Connected => {
                    return Err(VpnError::AlreadyActive);
                }
                ConnectionState::Stopping => {
                    return Err(VpnError::InvalidTransition(
                        "Stopping".to_string(),
                        "Starting".to_string(),
                    ));
                }
            }
        }

        if let Ok(mut lock) = self.last_error.write() {
            *lock = None;
        }
        self.log_manager
            .add_log("info", "Initiating VPN connection...");

        let cfg = self.config_manager.get_config();
        let profile = match cfg
            .active_profile_id
            .as_deref()
            .and_then(|id| self.profile_manager.get(id))
        {
            Some(p) => p,
            None => {
                let err_msg = "Cannot connect: No active server profile selected".to_string();
                self.set_error(&err_msg);
                return Err(VpnError::NoActiveProfile);
            }
        };

        let rules = self.routing_manager.get_rules();
        let singbox_config = self.core_manager.generate_singbox_config(
            &profile,
            cfg.routing_mode,
            cfg.default_route_action,
            &rules,
            &cfg.remote_dns,
            &cfg.direct_dns,
            &cfg.tun_interface_name,
            &cfg.log_level,
        );

        match self.core_manager.start_core(singbox_config).await {
            Ok(_) => {
                if let Ok(mut state) = self.state.write() {
                    *state = ConnectionState::Connected;
                }
                if let Ok(mut lock) = self.connected_since.write() {
                    *lock = Some(Utc::now());
                }
                self.log_manager.add_log(
                    "info",
                    &format!("VPN connected successfully to '{}'", profile.name),
                );
                Ok(())
            }
            Err(e) => {
                let err_msg = format!("VPN core start failed: {}", e);
                self.set_error(&err_msg);
                Err(VpnError::Core(e.to_string()))
            }
        }
    }

    pub async fn disconnect(&self) -> Result<(), VpnError> {
        {
            let mut state = self
                .state
                .write()
                .map_err(|e| VpnError::Lock(e.to_string()))?;
            match *state {
                ConnectionState::Connected | ConnectionState::Starting => {
                    *state = ConnectionState::Stopping;
                }
                ConnectionState::Disconnected => {
                    return Ok(());
                }
                ConnectionState::Stopping => {
                    return Ok(());
                }
                ConnectionState::Error => {
                    *state = ConnectionState::Disconnected;
                    if let Ok(mut cs) = self.connected_since.write() {
                        *cs = None;
                    }
                    return Ok(());
                }
            }
        }

        self.log_manager.add_log("info", "Disconnecting VPN...");

        let res = self.core_manager.stop_core().await;

        if let Ok(mut state) = self.state.write() {
            *state = ConnectionState::Disconnected;
        }
        if let Ok(mut cs) = self.connected_since.write() {
            *cs = None;
        }

        match res {
            Ok(_) => {
                self.log_manager.add_log("info", "VPN disconnected");
                Ok(())
            }
            Err(e) => {
                self.log_manager
                    .add_log("warn", &format!("Core stop returned: {}", e));
                Ok(())
            }
        }
    }

    pub async fn switch_profile(&self, new_profile_id: &str) -> Result<(), VpnError> {
        let is_connected = {
            let state = self.get_state();
            state == ConnectionState::Connected || state == ConnectionState::Starting
        };

        if is_connected {
            self.log_manager.add_log(
                "info",
                "Switching active profile on connected VPN: stopping current connection first...",
            );
            self.disconnect().await?;
        }

        let _ = self.config_manager.update(|c| {
            c.active_profile_id = Some(new_profile_id.to_string());
        });

        if is_connected {
            self.connect().await?;
        }

        Ok(())
    }

    fn set_error(&self, message: &str) {
        if let Ok(mut state) = self.state.write() {
            *state = ConnectionState::Error;
        }
        if let Ok(mut cs) = self.connected_since.write() {
            *cs = None;
        }
        if let Ok(mut err) = self.last_error.write() {
            *err = Some(message.to_string());
        }
        self.log_manager.add_log("error", message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn setup_test_manager() -> ConnectionManager {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let log_mgr = LogManager::default();
        let profile_mgr = ProfileManager::default();
        let routing_mgr = RoutingManager::default();
        let config_mgr = ConfigManager::new(path);
        let core_mgr = CoreManager::new(log_mgr.clone());

        ConnectionManager::new(profile_mgr, routing_mgr, config_mgr, core_mgr, log_mgr)
    }

    #[tokio::test]
    async fn test_initial_state_disconnected() {
        let mgr = setup_test_manager();
        assert_eq!(mgr.get_state(), ConnectionState::Disconnected);
        let status = mgr.get_status();
        assert_eq!(status.state, ConnectionState::Disconnected);
        assert!(status.connected_since.is_none());
    }

    #[tokio::test]
    async fn test_connect_without_profile_fails_gracefully() {
        let mgr = setup_test_manager();
        let res = mgr.connect().await;
        assert!(matches!(res, Err(VpnError::NoActiveProfile)));
        assert_eq!(mgr.get_state(), ConnectionState::Error);
        assert!(mgr.get_status().error_message.is_some());
    }

    #[tokio::test]
    async fn test_repeated_connect_prevented() {
        let mgr = setup_test_manager();
        *mgr.state.write().unwrap() = ConnectionState::Starting;
        let res = mgr.connect().await;
        assert_eq!(res, Err(VpnError::AlreadyActive));
    }

    #[tokio::test]
    async fn test_disconnect_from_error_resets_to_disconnected() {
        let mgr = setup_test_manager();
        *mgr.state.write().unwrap() = ConnectionState::Error;
        mgr.disconnect().await.unwrap();
        assert_eq!(mgr.get_state(), ConnectionState::Disconnected);
    }
}
