use crate::config::RoutingMode;
use crate::logging::LogManager;
use crate::profiles::{Profile, ProtocolConfig};
use crate::routing::{RoutingRule, RuleAction};
use crate::tun::TunManager;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("sing-box binary not found")]
    BinaryNotFound,
    #[error("sing-box check failed: {0}")]
    ValidationFailed(String),
    #[error("Core is already running")]
    AlreadyRunning,
    #[error("Core is not running")]
    NotRunning,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to serialize config: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Lock error: {0}")]
    Lock(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreStatus {
    pub is_running: bool,
    pub pid: Option<u32>,
    pub version: Option<String>,
    pub binary_path: Option<String>,
}

pub struct CoreProcess {
    pub child: Child,
    pub pid: u32,
    pub config_path: PathBuf,
}

#[derive(Clone)]
pub struct CoreManager {
    process: Arc<Mutex<Option<CoreProcess>>>,
    custom_binary_path: Arc<Mutex<Option<PathBuf>>>,
    cached_version: Arc<Mutex<Option<String>>>,
    log_manager: LogManager,
    tun_manager: TunManager,
}

impl CoreManager {
    pub fn new(log_manager: LogManager) -> Self {
        Self {
            process: Arc::new(Mutex::new(None)),
            custom_binary_path: Arc::new(Mutex::new(None)),
            cached_version: Arc::new(Mutex::new(None)),
            log_manager,
            tun_manager: TunManager::new(),
        }
    }

    pub fn set_custom_binary_path(&self, path: Option<PathBuf>) {
        if let Ok(mut lock) = self.custom_binary_path.lock() {
            *lock = path;
        }
    }

    pub fn find_binary(&self) -> Option<PathBuf> {
        if let Ok(lock) = self.custom_binary_path.lock() {
            if let Some(ref path) = *lock {
                if path.exists() {
                    return Some(path.clone());
                } else {
                    return None;
                }
            }
        }

        let exe_name = if cfg!(target_os = "windows") {
            "sing-box.exe"
        } else {
            "sing-box"
        };

        if let Ok(current_exe) = std::env::current_exe() {
            if let Some(parent) = current_exe.parent() {
                let candidate = parent.join(exe_name);
                if candidate.exists() {
                    return Some(candidate);
                }
                let candidate_bin = parent.join("bin").join(exe_name);
                if candidate_bin.exists() {
                    return Some(candidate_bin);
                }
            }
        }

        let local_bin = PathBuf::from("bin").join(exe_name);
        if local_bin.exists() {
            return Some(local_bin);
        }
        let local_root = PathBuf::from(exe_name);
        if local_root.exists() {
            return Some(local_root);
        }

        if let Ok(path) = which::which(exe_name) {
            return Some(path);
        }

        None
    }

    pub fn get_version(&self) -> Result<String, CoreError> {
        if let Ok(lock) = self.cached_version.lock() {
            if let Some(ref v) = *lock {
                return Ok(v.clone());
            }
        }

        let binary = self.find_binary().ok_or(CoreError::BinaryNotFound)?;
        let mut cmd = std::process::Command::new(&binary);
        cmd.arg("version");
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        let output = cmd.output()?;

        if output.status.success() {
            let ver = String::from_utf8_lossy(&output.stdout);
            let result = ver.lines().next().unwrap_or("sing-box").trim().to_string();
            if let Ok(mut lock) = self.cached_version.lock() {
                *lock = Some(result.clone());
            }
            Ok(result)
        } else {
            let err = String::from_utf8_lossy(&output.stderr);
            Err(CoreError::ValidationFailed(err.to_string()))
        }
    }

    pub fn get_status(&self) -> CoreStatus {
        let lock = self.process.lock().ok();
        let is_running = lock.as_ref().map(|l| l.is_some()).unwrap_or(false);
        let pid = lock.as_ref().and_then(|l| l.as_ref().map(|p| p.pid));
        let binary_path = self.find_binary().map(|p| p.display().to_string());
        let version = self.get_version().ok();

        CoreStatus {
            is_running,
            pid,
            version,
            binary_path,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn generate_singbox_config(
        &self,
        profile: &Profile,
        routing_mode: RoutingMode,
        default_action: RuleAction,
        rules: &[RoutingRule],
        remote_dns: &str,
        _direct_dns: &str,
        tun_interface_name: &str,
        log_level: &str,
    ) -> serde_json::Value {
        let proxy_outbound = match &profile.protocol {
            ProtocolConfig::Vless(cfg) => {
                let mut vless_json = json!({
                    "type": "vless",
                    "tag": "proxy",
                    "server": profile.server,
                    "server_port": profile.port,
                    "uuid": cfg.uuid,
                    "packet_encoding": "xudp"
                });

                if let Some(ref flow) = cfg.flow {
                    vless_json["flow"] = json!(flow);
                }

                if cfg.tls {
                    let mut tls_json = json!({
                        "enabled": true
                    });
                    if let Some(ref sni) = cfg.sni {
                        tls_json["server_name"] = json!(sni);
                    }
                    if let Some(ref fp) = cfg.fingerprint {
                        tls_json["utls"] = json!({
                            "enabled": true,
                            "fingerprint": fp
                        });
                    }
                    if cfg.reality {
                        let mut reality_json = json!({
                            "enabled": true
                        });
                        if let Some(ref pbk) = cfg.public_key {
                            reality_json["public_key"] = json!(pbk);
                        }
                        if let Some(ref sid) = cfg.short_id {
                            reality_json["short_id"] = json!(sid);
                        }
                        tls_json["reality"] = reality_json;
                    }
                    vless_json["tls"] = tls_json;
                }

                if let Some(ref transport) = cfg.network {
                    if transport == "ws" {
                        vless_json["transport"] = json!({ "type": "ws" });
                    } else if transport == "grpc" {
                        vless_json["transport"] = json!({ "type": "grpc" });
                    }
                }

                vless_json
            }
            ProtocolConfig::Shadowsocks(cfg) => {
                json!({
                    "type": "shadowsocks",
                    "tag": "proxy",
                    "server": profile.server,
                    "server_port": profile.port,
                    "method": cfg.method,
                    "password": cfg.password
                })
            }
        };

        let direct_outbound = json!({
            "type": "direct",
            "tag": "direct"
        });

        let mut dns_rules = Vec::new();
        let mut route_rules = Vec::new();

        route_rules.push(json!({ "action": "sniff" }));
        route_rules.push(json!({ "port": 53, "action": "hijack-dns" }));
        route_rules.push(json!({ "protocol": "dns", "action": "hijack-dns" }));
        route_rules.push(json!({ "ip_is_private": true, "outbound": "direct" }));

        // Bypass proxy server itself to prevent routing loops
        route_rules.push(json!({
            "domain": [profile.server.clone()],
            "outbound": "direct"
        }));
        dns_rules.push(json!({
            "domain": [profile.server.clone()],
            "server": "dns-direct"
        }));

        match routing_mode {
            RoutingMode::GlobalVpn => {}
            RoutingMode::GlobalDirect => {}
            RoutingMode::DomainRules => {
                for rule in rules {
                    if !rule.enabled {
                        continue;
                    }
                    let (rule_type, pattern, outbound) = rule.to_singbox_rule_target();
                    let dns_server = if outbound == "direct" {
                        "dns-direct"
                    } else {
                        "dns-remote"
                    };

                    dns_rules.push(json!({
                        rule_type: [pattern],
                        "server": dns_server
                    }));

                    route_rules.push(json!({
                        rule_type: [pattern],
                        "outbound": outbound
                    }));
                }
            }
        }

        let final_outbound = match routing_mode {
            RoutingMode::GlobalVpn => "proxy",
            RoutingMode::GlobalDirect => "direct",
            RoutingMode::DomainRules => match default_action {
                RuleAction::Vpn => "proxy",
                RuleAction::Direct => "direct",
            },
        };

        let final_dns = if final_outbound == "direct" {
            "dns-direct"
        } else {
            "dns-remote"
        };

        let tun_inbound = self.tun_manager.generate_tun_inbound(tun_interface_name);

        let remote_dns_val = if remote_dns.is_empty() {
            "1.1.1.1"
        } else {
            remote_dns
        };

        let direct_dns_val = if _direct_dns.is_empty() || _direct_dns == "local" {
            "77.88.8.8"
        } else {
            _direct_dns
        };

        json!({
            "log": {
                "level": log_level,
                "timestamp": true
            },
            "dns": {
                "servers": [
                    {
                        "tag": "dns-remote",
                        "type": "udp",
                        "server": remote_dns_val,
                        "detour": "proxy"
                    },
                    {
                        "tag": "dns-direct",
                        "type": "udp",
                        "server": direct_dns_val
                    }
                ],
                "rules": dns_rules,
                "final": final_dns
            },
            "inbounds": [tun_inbound],
            "outbounds": [proxy_outbound, direct_outbound],
            "route": {
                "auto_detect_interface": true,
                "default_domain_resolver": "dns-direct",
                "rules": route_rules,
                "final": final_outbound
            }
        })
    }

    pub fn validate_config(&self, config: &serde_json::Value) -> Result<(), CoreError> {
        let binary = self.find_binary().ok_or(CoreError::BinaryNotFound)?;
        let temp_dir = std::env::temp_dir();
        let temp_config = temp_dir.join(format!("rustybox_check_{}.json", uuid::Uuid::new_v4()));

        let json_str = serde_json::to_string_pretty(config)?;
        std::fs::write(&temp_config, json_str)?;

        let mut cmd = std::process::Command::new(&binary);
        cmd.arg("check").arg("-c").arg(&temp_config);
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        let output = cmd.output();

        let _ = std::fs::remove_file(&temp_config);

        match output {
            Ok(out) => {
                if out.status.success() {
                    Ok(())
                } else {
                    let err = String::from_utf8_lossy(&out.stderr);
                    Err(CoreError::ValidationFailed(err.to_string()))
                }
            }
            Err(e) => Err(CoreError::Io(e)),
        }
    }

    pub async fn start_core(&self, config: serde_json::Value) -> Result<u32, CoreError> {
        {
            let lock = self
                .process
                .lock()
                .map_err(|e| CoreError::Lock(e.to_string()))?;
            if lock.is_some() {
                return Err(CoreError::AlreadyRunning);
            }
        }

        let binary = self.find_binary().ok_or(CoreError::BinaryNotFound)?;
        self.validate_config(&config)?;

        let temp_dir = std::env::temp_dir();
        let config_path = temp_dir.join(format!("rustybox_run_{}.json", uuid::Uuid::new_v4()));
        let json_str = serde_json::to_string_pretty(&config)?;
        std::fs::write(&config_path, json_str)?;

        self.log_manager.add_log(
            "info",
            &format!("Starting sing-box core with binary {}", binary.display()),
        );

        let mut cmd = Command::new(&binary);
        cmd.arg("run")
            .arg("-c")
            .arg(&config_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        #[cfg(target_os = "windows")]
        {
            cmd.creation_flags(0x08000000);
        }

        let mut child = cmd.spawn()?;
        let pid = child.id().unwrap_or(0);

        if let Some(stdout) = child.stdout.take() {
            let log_mgr = self.log_manager.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    log_mgr.add_log("info", &line);
                }
            });
        }

        if let Some(stderr) = child.stderr.take() {
            let log_mgr = self.log_manager.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    log_mgr.add_log("error", &line);
                }
            });
        }

        {
            let mut lock = self
                .process
                .lock()
                .map_err(|e| CoreError::Lock(e.to_string()))?;
            *lock = Some(CoreProcess {
                child,
                pid,
                config_path,
            });
        }

        self.log_manager.add_log(
            "info",
            &format!("sing-box core started successfully (PID: {})", pid),
        );

        Ok(pid)
    }

    pub async fn stop_core(&self) -> Result<(), CoreError> {
        let mut proc_opt = {
            let mut lock = self
                .process
                .lock()
                .map_err(|e| CoreError::Lock(e.to_string()))?;
            lock.take()
        };

        if let Some(mut proc) = proc_opt.take() {
            self.log_manager
                .add_log("info", "Stopping sing-box core process...");
            let _ = proc.child.kill().await;
            let _ = std::fs::remove_file(&proc.config_path);
            self.log_manager
                .add_log("info", "sing-box core process stopped");
            Ok(())
        } else {
            Err(CoreError::NotRunning)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::{ShadowsocksConfig, VlessConfig};

    #[test]
    fn test_generate_vless_singbox_config() {
        let log_mgr = LogManager::default();
        let core_mgr = CoreManager::new(log_mgr);

        let vless_cfg = VlessConfig {
            uuid: "b831381d-6324-4d53-ad4f-8cda98b3f811".to_string(),
            flow: Some("xtls-rprx-vision".to_string()),
            network: Some("tcp".to_string()),
            tls: true,
            sni: Some("example.com".to_string()),
            fingerprint: Some("chrome".to_string()),
            reality: true,
            public_key: Some("enlGdIoYJfgX5TRxi5iQnDDFG-6TVwrPLtj00OdXvlM".to_string()),
            short_id: Some("1234".to_string()),
        };
        let profile = Profile::new_vless(
            "Test Reality".to_string(),
            "1.2.3.4".to_string(),
            443,
            vless_cfg,
        );

        let rule1 = RoutingRule::new("direct.example.org", RuleAction::Direct);
        let rule2 = RoutingRule::new("*.vpn.example.org", RuleAction::Vpn);

        let config = core_mgr.generate_singbox_config(
            &profile,
            RoutingMode::DomainRules,
            RuleAction::Vpn,
            &[rule1, rule2],
            "1.1.1.1",
            "local",
            "rustybox-tun",
            "info",
        );

        assert_eq!(config["dns"]["servers"][0]["tag"], "dns-remote");
        assert_eq!(config["outbounds"][0]["type"], "vless");
        assert_eq!(
            config["outbounds"][0]["uuid"],
            "b831381d-6324-4d53-ad4f-8cda98b3f811"
        );
        assert_eq!(config["route"]["final"], "proxy");
        assert_eq!(config["route"]["rules"][0]["action"], "sniff");
        assert_eq!(config["route"]["rules"][1]["action"], "hijack-dns");

        if core_mgr.find_binary().is_some() {
            let val_res = core_mgr.validate_config(&config);
            assert!(val_res.is_ok(), "Validation failed: {:?}", val_res.err());
        }
    }

    #[test]
    fn test_generate_shadowsocks_singbox_config() {
        let log_mgr = LogManager::default();
        let core_mgr = CoreManager::new(log_mgr);

        let ss_cfg = ShadowsocksConfig {
            method: "chacha20-ietf-poly1305".to_string(),
            password: "mypassword123".to_string(),
        };
        let profile =
            Profile::new_shadowsocks("Test SS".to_string(), "9.9.9.9".to_string(), 8388, ss_cfg);

        let config = core_mgr.generate_singbox_config(
            &profile,
            RoutingMode::GlobalVpn,
            RuleAction::Vpn,
            &[],
            "8.8.8.8",
            "local",
            "rustybox-tun",
            "info",
        );

        assert_eq!(config["outbounds"][0]["type"], "shadowsocks");
        assert_eq!(config["outbounds"][0]["method"], "chacha20-ietf-poly1305");
        assert_eq!(config["outbounds"][0]["password"], "mypassword123");

        if core_mgr.find_binary().is_some() {
            let val_res = core_mgr.validate_config(&config);
            assert!(val_res.is_ok(), "Validation failed: {:?}", val_res.err());
        }
    }
}
