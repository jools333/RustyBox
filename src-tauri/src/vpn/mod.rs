use crate::config::{ConfigManager, DnsMode, RoutingMode};
use crate::core::CoreManager;
use crate::logging::LogManager;
use crate::profiles::{Profile, ProfileManager, ProtocolConfig};
use crate::routing::{RoutingManager, RuleAction};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;
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
    #[serde(default)]
    pub work_profile: Option<Profile>,
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
    health_stop: Arc<RwLock<Option<Arc<AtomicBool>>>>,
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
            health_stop: Arc::new(RwLock::new(None)),
        }
    }

    pub fn get_state(&self) -> ConnectionState {
        self.state
            .read()
            .map(|s| s.clone())
            .unwrap_or(ConnectionState::Error)
    }

    pub fn get_status(&self) -> ConnectionStatus {
        let mut state = self.get_state();

        // The core can die on its own (TUN setup failure, server unreachable in
        // a fatal way, crash). Without this check the UI keeps showing
        // "Connected" while the system silently falls back to the ISP route.
        if state == ConnectionState::Connected && !self.core_manager.is_running() {
            let reason = self.core_manager.take_exit_error().unwrap_or_else(|| {
                "sing-box core stopped unexpectedly, traffic is no longer routed through the VPN"
                    .to_string()
            });
            self.set_error(&format!("VPN disconnected unexpectedly: {}", reason));
            state = ConnectionState::Error;
        }

        let connected_since = self.connected_since.read().ok().and_then(|t| *t);
        let error_message = self.last_error.read().ok().and_then(|e| e.clone());
        let cfg = self.config_manager.get_config();
        let active_profile = cfg
            .active_profile_id
            .as_deref()
            .and_then(|id| self.profile_manager.get(id));
        let work_profile = if cfg.work_profile_enabled {
            cfg.work_profile_id
                .as_deref()
                .filter(|id| Some(*id) != cfg.active_profile_id.as_deref())
                .and_then(|id| self.profile_manager.get(id))
        } else {
            None
        };

        ConnectionStatus {
            state,
            active_profile,
            work_profile,
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
        self.stop_health_watchdog();
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

        let work_profile = if cfg.work_profile_enabled {
            cfg.work_profile_id
                .as_deref()
                .filter(|id| Some(*id) != cfg.active_profile_id.as_deref())
                .and_then(|id| self.profile_manager.get(id))
        } else {
            None
        };

        // Validate OpenVPN certificate readiness
        if let ProtocolConfig::Openvpn(ref ovpn) = profile.protocol {
            if ovpn.ca_cert.is_none() && ovpn.tls_auth.is_none() && ovpn.tls_crypt.is_none() {
                let err_msg = format!(
                    "OpenVPN profile '{}' is missing CA certificate. Please re-import the .ovpn file using the 'Import .ovpn' button.",
                    profile.name
                );
                self.set_error(&err_msg);
                return Err(VpnError::Core(err_msg));
            }
        }
        if let Some(ref work) = work_profile {
            if let ProtocolConfig::Openvpn(ref ovpn) = work.protocol {
                if ovpn.ca_cert.is_none() && ovpn.tls_auth.is_none() && ovpn.tls_crypt.is_none() {
                    let err_msg = format!(
                        "Work OpenVPN profile '{}' is missing CA certificate. Please re-import the .ovpn file using the 'Import .ovpn' button.",
                        work.name
                    );
                    self.set_error(&err_msg);
                    return Err(VpnError::Core(err_msg));
                }
            }
        }

        if let Some(ref work) = work_profile {
            self.log_manager.add_log(
                "info",
                &format!(
                    "Dual-VPN enabled: Primary '{}' ({:?}) + Work '{}' ({:?})",
                    profile.name, profile.protocol, work.name, work.protocol
                ),
            );
        } else if profile.only_network_resources {
            self.log_manager.add_log(
                "info",
                &format!(
                    "VPN profile '{}' is in split-tunnel mode (only corporate network resources)",
                    profile.name
                ),
            );
        }

        // A hostname that cannot be resolved means the core's own traffic
        // cannot be excluded from the TUN.  Starting anyway creates a routing
        // loop; with DNS hijacking enabled it also takes the whole system's
        // name resolution down.  Fail fast with an actionable message.
        self.ensure_server_resolvable(&profile)?;
        if let Some(ref work) = work_profile {
            self.ensure_server_resolvable(work)?;
        }

        let rules = self.routing_manager.get_rules();
        let final_is_proxy = match cfg.routing_mode {
            RoutingMode::GlobalVpn => true,
            RoutingMode::GlobalDirect => false,
            RoutingMode::DomainRules => cfg.default_route_action == RuleAction::Vpn,
        };

        self.log_manager.add_log(
            "info",
            &format!(
                "DNS mode: {} (LAN DNS stays outside the tunnel: {}, health check: {})",
                cfg.dns_mode.as_str(),
                cfg.dns_mode != DnsMode::Vpn,
                cfg.health_check
            ),
        );

        let mut tun_ipv6 = cfg.tun_ipv6;
        let mut retried_without_ipv6 = false;
        loop {
            let singbox_config = self.core_manager.generate_singbox_config(
                &profile,
                work_profile.as_ref(),
                cfg.routing_mode,
                cfg.default_route_action,
                &rules,
                &cfg.remote_dns,
                &cfg.direct_dns,
                &cfg.tun_interface_name,
                cfg.tun_mtu,
                &cfg.log_level,
                cfg.dns_mode,
                tun_ipv6,
            );

            match self.core_manager.start_core(singbox_config).await {
                Ok(_) => break,
                Err(e) => {
                    let err_text = e.to_string();

                    // Some kernels / container / NetworkManager setups reject
                    // adding an IPv6 address to a TUN device with EPERM.  The
                    // VPN works fine over IPv4, so retry once without IPv6.
                    if tun_ipv6
                        && !retried_without_ipv6
                        && err_text.contains("add address")
                        && err_text.contains("permission denied")
                    {
                        self.log_manager.add_log(
                            "warn",
                            "Could not add the IPv6 address to the TUN (permission denied); retrying without IPv6",
                        );
                        tun_ipv6 = false;
                        retried_without_ipv6 = true;
                        let _ = self.core_manager.stop_core().await;
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        continue;
                    }

                    let err_msg = format!("VPN core start failed: {}", e);
                    self.set_error(&err_msg);
                    return Err(VpnError::Core(err_text));
                }
            }
        }

        // Drop any negative/cached answers from before the connection.  This
        // is especially important when the previous attempt failed DNS.
        flush_dns_cache();

        // Do not report "Connected" while the tunnel is silently dead.  This is
        // exactly the failure mode where sing-box is running but its upstream
        // server stopped passing traffic: pages hang and DNS dies.
        if cfg.health_check {
            self.log_manager.add_log("info", "Checking VPN health...");
            let mut healthy = false;
            for attempt in 1..=2 {
                tokio::time::sleep(Duration::from_secs(if attempt == 1 { 3 } else { 4 })).await;
                if self.health_check(final_is_proxy, cfg.dns_mode).await {
                    healthy = true;
                    break;
                }
                self.log_manager.add_log(
                    "warn",
                    &format!("VPN health check failed (attempt {}/2)", attempt),
                );
            }

            if !healthy {
                let _ = self.core_manager.stop_core().await;
                flush_dns_cache();
                let msg = "VPN connected, but the tunnel is not passing traffic or DNS does not work. Disconnected to restore direct network access.";
                self.set_error(msg);
                return Err(VpnError::Core(msg.to_string()));
            }
        }

        if let Ok(mut state) = self.state.write() {
            *state = ConnectionState::Connected;
        }
        if let Ok(mut lock) = self.connected_since.write() {
            *lock = Some(Utc::now());
        }
        if let Some(ref work) = work_profile {
            self.log_manager.add_log(
                "info",
                &format!(
                    "Dual-VPN connected: Primary '{}' + Work '{}'",
                    profile.name, work.name
                ),
            );
        } else {
            self.log_manager.add_log(
                "info",
                &format!("VPN connected successfully to '{}'", profile.name),
            );
        }

        self.start_health_watchdog(final_is_proxy, cfg.dns_mode, cfg.health_check);
        Ok(())
    }

    fn ensure_server_resolvable(&self, profile: &Profile) -> Result<(), VpnError> {
        if profile.server.parse::<std::net::IpAddr>().is_ok() {
            return Ok(());
        }

        if CoreManager::server_exclude_cidrs(&profile.server).is_empty() {
            let msg = format!(
                "Cannot resolve VPN server '{}'. Check DNS settings or use the server IP address in the profile.",
                profile.server
            );
            self.set_error(&msg);
            return Err(VpnError::Core(msg));
        }

        Ok(())
    }

    async fn health_check(&self, final_is_proxy: bool, dns_mode: DnsMode) -> bool {
        // Test the tunnel itself with a TCP connection.  The TUN is the default
        // route, so this connection is proxied by sing-box; if the upstream is
        // dead it times out.  Probe several well-known destinations in
        // parallel to avoid a false positive when one is blocked server-side.
        if final_is_proxy {
            let tunnel_ok = tokio::task::spawn_blocking(|| {
                const TARGETS: [&str; 3] = ["1.1.1.1", "8.8.8.8", "77.88.8.8"];
                std::thread::scope(|scope| {
                    let handles: Vec<_> = TARGETS
                        .iter()
                        .map(|ip| {
                            scope.spawn(move || tcp_reachable(ip, 443, Duration::from_secs(4)))
                        })
                        .collect();
                    handles
                        .into_iter()
                        .any(|handle| handle.join().unwrap_or(false))
                })
            })
            .await
            .unwrap_or(false);

            if !tunnel_ok {
                return false;
            }
        }

        // In VPN-only DNS mode the system resolver depends on the tunnel.
        // In auto/direct mode LAN DNS is deliberately independent, so a
        // failure here would be a separate local-DNS problem.
        if dns_mode == DnsMode::Vpn {
            let dns_ok = tokio::time::timeout(
                Duration::from_secs(8),
                tokio::task::spawn_blocking(|| {
                    ("example.com", 0)
                        .to_socket_addrs()
                        .map(|mut addrs| addrs.next().is_some())
                        .unwrap_or(false)
                }),
            )
            .await;
            return matches!(dns_ok, Ok(Ok(true)));
        }

        true
    }

    fn stop_health_watchdog(&self) {
        if let Ok(mut guard) = self.health_stop.write() {
            if let Some(flag) = guard.take() {
                flag.store(true, Ordering::Relaxed);
            }
        }
    }

    fn start_health_watchdog(&self, final_is_proxy: bool, dns_mode: DnsMode, enabled: bool) {
        self.stop_health_watchdog();
        if !enabled {
            return;
        }

        let stop = Arc::new(AtomicBool::new(false));
        if let Ok(mut guard) = self.health_stop.write() {
            *guard = Some(stop.clone());
        }

        let this = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(20)).await;
            let mut failures = 0u8;
            while !stop.load(Ordering::Relaxed) {
                if this.get_state() != ConnectionState::Connected {
                    break;
                }

                if this.health_check(final_is_proxy, dns_mode).await {
                    failures = 0;
                } else {
                    failures += 1;
                    this.log_manager
                        .add_log("warn", &format!("VPN health check failed ({}/2)", failures));
                    if failures >= 2 {
                        this.log_manager.add_log(
                            "error",
                            "VPN tunnel stopped passing traffic; disconnecting to restore direct DNS and internet access",
                        );
                        let _ = this.disconnect().await;
                        this.set_error(
                            "VPN tunnel stopped passing traffic; disconnected automatically to restore DNS/internet",
                        );
                        break;
                    }
                }

                tokio::time::sleep(Duration::from_secs(20)).await;
            }
        });
    }

    pub async fn disconnect(&self) -> Result<(), VpnError> {
        self.stop_health_watchdog();
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
        flush_dns_cache();

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

    pub async fn reconnect(&self) -> Result<(), VpnError> {
        let is_connected = {
            let state = self.get_state();
            state == ConnectionState::Connected || state == ConnectionState::Starting
        };

        if is_connected {
            self.log_manager
                .add_log("info", "Reconnecting VPN to apply configuration updates...");
            self.disconnect().await?;
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

/// Best-effort DNS cache flush.  After a failed VPN session the system resolver
/// may keep negative answers for a while, making it look like DNS is still
/// broken even though the tunnel is already gone.
fn flush_dns_cache() {
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("resolvectl")
            .arg("flush-caches")
            .status();
    }

    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("ipconfig")
            .arg("/flushdns")
            .status();
    }

    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("dscacheutil")
            .arg("-flushcache")
            .status();
        let _ = std::process::Command::new("killall")
            .args(["-HUP", "mDNSResponder"])
            .status();
    }
}

/// Best-effort TCP reachability check used by the health watchdog.
///
/// The address is deliberately parsed as a literal so this never performs a
/// DNS lookup itself (a dead DNS server must not make the health check hang).
fn tcp_reachable(ip: &str, port: u16, timeout: Duration) -> bool {
    let addr: SocketAddr = match format!("{}:{}", ip, port).parse() {
        Ok(addr) => addr,
        Err(_) => return false,
    };
    TcpStream::connect_timeout(&addr, timeout).is_ok()
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

    #[tokio::test]
    async fn test_connected_state_detects_dead_core() {
        let mgr = setup_test_manager();
        *mgr.state.write().unwrap() = ConnectionState::Connected;

        // No core process exists, so the status must not keep reporting Connected.
        let status = mgr.get_status();
        assert_eq!(status.state, ConnectionState::Error);
        assert!(status
            .error_message
            .as_deref()
            .unwrap_or_default()
            .contains("unexpectedly"));
        assert!(status.connected_since.is_none());
    }
}
