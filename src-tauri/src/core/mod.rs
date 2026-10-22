use crate::config::RoutingMode;
use crate::logging::LogManager;
use crate::profiles::{Profile, ProtocolConfig};
use crate::routing::{RoutingRule, RuleAction};
use crate::tun::TunManager;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("sing-box binary not found")]
    BinaryNotFound,
    #[error("sing-box check failed: {0}")]
    ValidationFailed(String),
    #[error("sing-box failed to start: {0}")]
    StartFailed(String),
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
    resource_dir: Arc<Mutex<Option<PathBuf>>>,
    cached_version: Arc<Mutex<Option<String>>>,
    recent_output: Arc<Mutex<VecDeque<String>>>,
    last_exit_error: Arc<Mutex<Option<String>>>,
    log_manager: LogManager,
    tun_manager: TunManager,
}

/// Strips ANSI terminal escape codes from text (e.g. \x1b[36m, \x1b[0m).
pub fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                while let Some(&next_ch) = chars.peek() {
                    chars.next();
                    if next_ch.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            out.push(ch);
        }
    }
    out
}

pub fn format_diagnostic_lines(lines: &[String]) -> String {
    let clean: Vec<String> = lines
        .iter()
        .map(|line| strip_ansi(line))
        .map(|line| crate::logging::mask_secrets(&line))
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect();

    if clean.is_empty() {
        return String::new();
    }

    // Look for FATAL or ERROR lines first, as they contain the actual root cause
    let error_lines: Vec<&String> = clean
        .iter()
        .filter(|line| line.contains("FATAL") || line.contains("ERROR"))
        .collect();

    if !error_lines.is_empty() {
        let start = error_lines.len().saturating_sub(2);
        error_lines[start..]
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        let start = clean.len().saturating_sub(2);
        clean[start..]
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Maps a sing-box log line to an application log level.
///
/// sing-box writes all of its logs (including INFO) to stderr, so the stream
/// alone is not a reliable level indicator.
fn detect_level(line: &str) -> &'static str {
    if line.contains("FATAL") || line.contains("ERROR") {
        "error"
    } else if line.contains("WARN") {
        "warn"
    } else if line.contains("DEBUG") {
        "debug"
    } else {
        "info"
    }
}

#[cfg(unix)]
fn ensure_executable(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(metadata) = std::fs::metadata(path) {
        let mut perms = metadata.permissions();
        let mode = perms.mode();
        if mode & 0o111 == 0 {
            perms.set_mode(mode | 0o755);
            let _ = std::fs::set_permissions(path, perms);
        }
    }
}

#[cfg(not(unix))]
fn ensure_executable(_path: &std::path::Path) {}

/// Extracts (major, minor, patch) from a version string like "sing-box version 1.14.3" or "v1.14.0".
pub fn parse_version_tuple(version_str: &str) -> Option<(u64, u64, u64)> {
    let ver_part = if let Some(idx) = version_str.find("version ") {
        &version_str[idx + "version ".len()..]
    } else {
        version_str
    };

    let token = ver_part.split_whitespace().next()?;
    let clean = token.strip_prefix('v').unwrap_or(token);
    let num_part = clean.split('-').next().unwrap_or(clean);
    let parts: Vec<&str> = num_part.split('.').collect();
    if parts.len() >= 2 {
        let major = parts[0].parse::<u64>().ok()?;
        let minor = parts[1].parse::<u64>().ok()?;
        let patch = if parts.len() >= 3 {
            parts[2].parse::<u64>().unwrap_or(0)
        } else {
            0
        };
        Some((major, minor, patch))
    } else {
        None
    }
}

/// Validates that the sing-box binary meets the minimum version requirement (>= 1.14.0).
pub fn validate_singbox_version(
    version_str: &str,
    binary_path: &std::path::Path,
) -> Result<(), CoreError> {
    if let Some((major, minor, _)) = parse_version_tuple(version_str) {
        if major < 1 || (major == 1 && minor < 14) {
            return Err(CoreError::ValidationFailed(format!(
                "Incompatible sing-box version detected (found '{}' at {}). RustyBox requires sing-box v1.14.0 or newer. Please use the bundled sing-box or upgrade your system binary.",
                version_str,
                binary_path.display()
            )));
        }
    }
    Ok(())
}

impl CoreManager {
    pub fn new(log_manager: LogManager) -> Self {
        Self {
            process: Arc::new(Mutex::new(None)),
            custom_binary_path: Arc::new(Mutex::new(None)),
            resource_dir: Arc::new(Mutex::new(None)),
            cached_version: Arc::new(Mutex::new(None)),
            recent_output: Arc::new(Mutex::new(VecDeque::new())),
            last_exit_error: Arc::new(Mutex::new(None)),
            log_manager,
            tun_manager: TunManager::new(),
        }
    }

    fn recent_output_text(&self) -> String {
        self.recent_output
            .lock()
            .map(|recent| {
                let lines: Vec<String> = recent.iter().cloned().collect();
                format_diagnostic_lines(&lines)
            })
            .unwrap_or_default()
    }

    /// Returns true when the sing-box child process is still alive.
    ///
    /// Also detects a process that exited on its own (TUN setup failure, bad
    /// profile, crash, ...) and records a diagnostic message so the UI does not
    /// keep reporting "Connected" while traffic goes out directly through the
    /// ISP.
    pub fn is_running(&self) -> bool {
        let mut lock = match self.process.lock() {
            Ok(lock) => lock,
            Err(_) => return false,
        };

        let proc = match lock.as_mut() {
            Some(proc) => proc,
            None => return false,
        };

        match proc.child.try_wait() {
            Ok(None) => true,
            Ok(Some(status)) => {
                let output = self.recent_output_text();
                let message = if output.is_empty() {
                    format!("sing-box core exited unexpectedly ({})", status)
                } else {
                    format!("sing-box core exited unexpectedly ({}): {}", status, output)
                };
                self.log_manager.add_log("error", &message);
                if let Ok(mut last) = self.last_exit_error.lock() {
                    *last = Some(message);
                }
                let _ = std::fs::remove_file(&proc.config_path);
                *lock = None;
                false
            }
            Err(_) => false,
        }
    }

    /// Takes the most recent unexpected-exit diagnostic, if any.
    pub fn take_exit_error(&self) -> Option<String> {
        self.last_exit_error
            .lock()
            .ok()
            .and_then(|mut last| last.take())
    }

    pub fn set_resource_dir(&self, dir: PathBuf) {
        if let Ok(mut lock) = self.resource_dir.lock() {
            *lock = Some(dir);
        }
    }

    pub fn set_custom_binary_path(&self, path: Option<PathBuf>) {
        if let Ok(mut lock) = self.custom_binary_path.lock() {
            *lock = path;
        }
    }

    pub fn find_binary(&self) -> Option<PathBuf> {
        // 1. Explicit custom path set by user in config
        if let Ok(lock) = self.custom_binary_path.lock() {
            if let Some(ref path) = *lock {
                if path.exists() {
                    ensure_executable(path);
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

        // 2. Official Tauri resource directory (bundled with application)
        if let Ok(lock) = self.resource_dir.lock() {
            if let Some(ref res_dir) = *lock {
                let candidates = [res_dir.join("bin").join(exe_name), res_dir.join(exe_name)];
                for candidate in candidates {
                    if candidate.exists() {
                        ensure_executable(&candidate);
                        return Some(candidate);
                    }
                }
            }
        }

        // 3. Locations relative to current executable (portable, installed deb, AppImage, dev)
        if let Ok(current_exe) = std::env::current_exe() {
            if let Some(parent) = current_exe.parent() {
                let mut candidates = vec![
                    parent.join(exe_name),
                    parent.join("bin").join(exe_name),
                    parent.join("resources").join(exe_name),
                    parent.join("resources").join("bin").join(exe_name),
                    parent.join("_up_").join("bin").join(exe_name),
                    parent.join("_up_").join(exe_name),
                ];

                // Check grandparent paths (e.g. current_exe = /usr/bin/rustybox -> /usr/lib/rustybox/bin/sing-box)
                if let Some(grandparent) = parent.parent() {
                    candidates.push(
                        grandparent
                            .join("lib")
                            .join("rustybox")
                            .join("bin")
                            .join(exe_name),
                    );
                    candidates.push(
                        grandparent
                            .join("lib")
                            .join("RustyBox")
                            .join("bin")
                            .join(exe_name),
                    );
                    candidates.push(grandparent.join("lib").join("rustybox").join(exe_name));
                    candidates.push(grandparent.join("lib").join("RustyBox").join(exe_name));
                    candidates.push(
                        grandparent
                            .join("share")
                            .join("rustybox")
                            .join("bin")
                            .join(exe_name),
                    );
                    candidates.push(
                        grandparent
                            .join("share")
                            .join("RustyBox")
                            .join("bin")
                            .join(exe_name),
                    );

                    // Check great-grandparent (e.g. target/debug/rustybox -> <repo>/bin/sing-box)
                    if let Some(greatgrandparent) = grandparent.parent() {
                        candidates.push(greatgrandparent.join("bin").join(exe_name));
                    }
                }

                for candidate in candidates {
                    if candidate.exists() {
                        ensure_executable(&candidate);
                        return Some(candidate);
                    }
                }
            }
        }

        // 4. Linux standard package locations
        #[cfg(not(target_os = "windows"))]
        {
            let linux_system_candidates = [
                PathBuf::from("/usr/lib/rustybox/bin").join(exe_name),
                PathBuf::from("/usr/lib/RustyBox/bin").join(exe_name),
                PathBuf::from("/usr/lib/rustybox").join(exe_name),
                PathBuf::from("/usr/lib/RustyBox").join(exe_name),
                PathBuf::from("/usr/share/rustybox/bin").join(exe_name),
                PathBuf::from("/usr/share/RustyBox/bin").join(exe_name),
                PathBuf::from("/opt/rustybox/bin").join(exe_name),
                PathBuf::from("/opt/RustyBox/bin").join(exe_name),
            ];
            for candidate in linux_system_candidates {
                if candidate.exists() {
                    ensure_executable(&candidate);
                    return Some(candidate);
                }
            }
        }

        // 5. Working directory / project root candidates (dev mode)
        let local_bin = PathBuf::from("bin").join(exe_name);
        if local_bin.exists() {
            ensure_executable(&local_bin);
            return Some(local_bin);
        }
        let local_root = PathBuf::from(exe_name);
        if local_root.exists() {
            ensure_executable(&local_root);
            return Some(local_root);
        }

        // 6. System PATH only as last resort
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
        if let Some(parent) = binary.parent() {
            cmd.current_dir(parent);
        }
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
        let is_running = self.is_running();
        let pid = if is_running {
            self.process
                .lock()
                .ok()
                .and_then(|lock| lock.as_ref().map(|p| p.pid))
        } else {
            None
        };
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
        let version = self.get_version()?;
        validate_singbox_version(&version, &binary)?;

        let temp_dir = std::env::temp_dir();
        let temp_config = temp_dir.join(format!("rustybox_check_{}.json", uuid::Uuid::new_v4()));

        let json_str = serde_json::to_string_pretty(config)?;
        std::fs::write(&temp_config, json_str)?;

        let mut cmd = std::process::Command::new(&binary);
        if let Some(parent) = binary.parent() {
            cmd.current_dir(parent);
        }
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

        if let Ok(mut recent) = self.recent_output.lock() {
            recent.clear();
        }
        if let Ok(mut last) = self.last_exit_error.lock() {
            *last = None;
        }

        self.log_manager.add_log(
            "info",
            &format!("Starting sing-box core with binary {}", binary.display()),
        );

        // Pre-flight cleanup: remove any orphaned or conflicting TUN adapter left over from crashes
        #[cfg(target_os = "windows")]
        {
            self.tun_manager.cleanup_stale_adapter("rustybox-tun");
        }

        let mut cmd = Command::new(&binary);
        if let Some(parent) = binary.parent() {
            cmd.current_dir(parent);
        }
        cmd.arg("run")
            .arg("-c")
            .arg(&config_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        #[cfg(target_os = "windows")]
        {
            // CREATE_NO_WINDOW (0x08000000) prevents console window popup
            // CREATE_NEW_PROCESS_GROUP (0x00000200) allows sending CTRL_BREAK for graceful shutdown
            cmd.creation_flags(0x08000000 | 0x00000200);
        }

        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(e) => {
                let _ = std::fs::remove_file(&config_path);
                return Err(CoreError::Io(e));
            }
        };
        let pid = child.id().unwrap_or(0);
        let started_flag = Arc::new(AtomicBool::new(false));

        if let Some(stdout) = child.stdout.take() {
            let log_mgr = self.log_manager.clone();
            let recent = self.recent_output.clone();
            let started = started_flag.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    if line.contains("sing-box started") {
                        started.store(true, Ordering::SeqCst);
                    }
                    if let Ok(mut recent) = recent.lock() {
                        if recent.len() >= 40 {
                            recent.pop_front();
                        }
                        recent.push_back(line.clone());
                    }
                    let level = detect_level(&line);
                    log_mgr.add_log(level, &line);
                }
            });
        }

        if let Some(stderr) = child.stderr.take() {
            let log_mgr = self.log_manager.clone();
            let recent = self.recent_output.clone();
            let started = started_flag.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    // sing-box reports readiness on stderr as well.
                    if line.contains("sing-box started") {
                        started.store(true, Ordering::SeqCst);
                    }
                    if let Ok(mut recent) = recent.lock() {
                        if recent.len() >= 40 {
                            recent.pop_front();
                        }
                        recent.push_back(line.clone());
                    }
                    let level = detect_level(&line);
                    log_mgr.add_log(level, &line);
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
                config_path: config_path.clone(),
            });
        }

        // sing-box creates the TUN interface and installs routes before it logs
        // "sing-box started". Wait for that marker (or an early exit) so the UI
        // never reports "Connected" while the core is dead and traffic silently
        // bypasses the tunnel.
        // Wintun driver setup timeout is 15 seconds; give it up to 18s before declaring timeout.
        let deadline = Instant::now() + Duration::from_secs(18);
        let mut startup_timed_out = false;
        loop {
            let exit_status = {
                let mut lock = self
                    .process
                    .lock()
                    .map_err(|e| CoreError::Lock(e.to_string()))?;
                match lock.as_mut() {
                    Some(proc) => proc
                        .child
                        .try_wait()
                        .map_err(CoreError::Io)?
                        .map(|status| status.to_string()),
                    // Stopped externally (e.g. user pressed disconnect).
                    None => Some("stopped".to_string()),
                }
            };

            if let Some(status) = exit_status {
                // Give the stdout/stderr readers a moment to drain the pipe so
                // the diagnostic contains sing-box's own error message.
                tokio::time::sleep(Duration::from_millis(200)).await;
                if let Ok(mut lock) = self.process.lock() {
                    *lock = None;
                }
                let _ = std::fs::remove_file(&config_path);
                let details = self.recent_output_text();
                let message = if details.is_empty() {
                    format!("sing-box exited during startup ({})", status)
                } else if details.contains("Access is denied") {
                    format!(
                        "sing-box exited during startup ({}): {}. Administrator privileges are required to configure the TUN interface on Windows. Please run RustyBox as Administrator.",
                        status, details
                    )
                } else {
                    format!("sing-box exited during startup ({}): {}", status, details)
                };
                self.log_manager.add_log("error", &message);
                return Err(CoreError::StartFailed(message));
            }

            if started_flag.load(Ordering::SeqCst) {
                break;
            }

            if Instant::now() >= deadline {
                startup_timed_out = true;
                break;
            }

            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        if startup_timed_out || !self.is_running() {
            let proc_opt = {
                let mut lock = self
                    .process
                    .lock()
                    .map_err(|e| CoreError::Lock(e.to_string()))?;
                lock.take()
            };
            if let Some(mut proc) = proc_opt {
                let _ = proc.child.kill().await;
            }
            let _ = std::fs::remove_file(&config_path);
            let details = self.recent_output_text();
            let message = if details.is_empty() {
                "sing-box startup timed out waiting for TUN readiness".to_string()
            } else {
                format!("sing-box startup timed out: {}", details)
            };
            self.log_manager.add_log("error", &message);
            return Err(CoreError::StartFailed(message));
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

            #[cfg(target_os = "windows")]
            {
                extern "system" {
                    fn GenerateConsoleCtrlEvent(dwCtrlEvent: u32, dwProcessGroupId: u32) -> i32;
                }
                const CTRL_BREAK_EVENT: u32 = 1;
                // Signal sing-box for graceful teardown (routes restore and Wintun adapter cleanup)
                unsafe {
                    GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, proc.pid);
                }
                // Wait up to 2.5s for graceful exit
                let graceful_exit =
                    tokio::time::timeout(Duration::from_millis(2500), proc.child.wait()).await;

                if graceful_exit.is_err() {
                    let _ = proc.child.kill().await;
                }
            }

            #[cfg(not(target_os = "windows"))]
            {
                let _ = proc.child.kill().await;
            }

            let _ = std::fs::remove_file(&proc.config_path);

            // Clean up any residual adapter
            #[cfg(target_os = "windows")]
            {
                self.tun_manager.cleanup_stale_adapter("rustybox-tun");
            }

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
    fn test_is_not_running_without_process() {
        let core_mgr = CoreManager::new(LogManager::default());
        assert!(!core_mgr.is_running());
        assert!(core_mgr.take_exit_error().is_none());
        assert!(!core_mgr.get_status().is_running);
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

    #[test]
    fn test_strip_ansi_and_diagnostic_formatting() {
        let raw = "+0300 2026-10-21 15:12:06 \x1b[36mINFO\x1b[0m network: updated\n\x1b[31mFATAL\x1b[0m[0015] start service: configure tun interface: (create adapter: Cannot create a file when that file already exists.)";
        let stripped = strip_ansi(raw);
        assert!(!stripped.contains("\x1b"));
        assert!(stripped.contains("INFO network: updated"));
        assert!(stripped.contains("FATAL[0015] start service: configure tun interface"));

        let lines = vec![
            "\x1b[36mINFO\x1b[0m normal log".to_string(),
            "\x1b[31mFATAL\x1b[0m[0015] start service: create adapter error".to_string(),
        ];
        let formatted = format_diagnostic_lines(&lines);
        assert_eq!(formatted, "FATAL[0015] start service: create adapter error");
    }

    #[test]
    fn test_custom_binary_path() {
        let core_mgr = CoreManager::new(LogManager::default());
        let temp = tempfile::NamedTempFile::new().unwrap();
        let path = temp.path().to_path_buf();
        core_mgr.set_custom_binary_path(Some(path.clone()));
        assert_eq!(core_mgr.find_binary(), Some(path));

        core_mgr.set_custom_binary_path(Some(PathBuf::from("/nonexistent/sing-box")));
        assert_eq!(core_mgr.find_binary(), None);
    }

    #[test]
    fn test_parse_version_tuple() {
        assert_eq!(
            parse_version_tuple("sing-box version 1.14.3"),
            Some((1, 14, 3))
        );
        assert_eq!(
            parse_version_tuple("sing-box version 1.11.0"),
            Some((1, 11, 0))
        );
        assert_eq!(
            parse_version_tuple("sing-box version 1.15.0-alpha.12"),
            Some((1, 15, 0))
        );
        assert_eq!(parse_version_tuple("v1.14.0"), Some((1, 14, 0)));
        assert_eq!(parse_version_tuple("invalid"), None);
    }

    #[test]
    fn test_validate_singbox_version() {
        let dummy_path = PathBuf::from("/bin/sing-box");
        assert!(validate_singbox_version("sing-box version 1.14.3", &dummy_path).is_ok());
        assert!(validate_singbox_version("sing-box version 1.14.0", &dummy_path).is_ok());
        assert!(validate_singbox_version("sing-box version 1.15.0", &dummy_path).is_ok());

        let res = validate_singbox_version("sing-box version 1.11.0", &dummy_path);
        assert!(res.is_err());
        let err_msg = res.unwrap_err().to_string();
        assert!(err_msg.contains("Incompatible sing-box version detected"));
        assert!(err_msg.contains("1.11.0"));
    }

    #[test]
    fn test_resource_dir_priority() {
        let core_mgr = CoreManager::new(LogManager::default());
        let temp_dir = tempfile::tempdir().unwrap();
        let exe_name = if cfg!(target_os = "windows") {
            "sing-box.exe"
        } else {
            "sing-box"
        };
        let dummy_bin = temp_dir.path().join("bin").join(exe_name);
        std::fs::create_dir_all(dummy_bin.parent().unwrap()).unwrap();
        std::fs::write(&dummy_bin, "fake-binary").unwrap();

        core_mgr.set_resource_dir(temp_dir.path().to_path_buf());
        assert_eq!(core_mgr.find_binary(), Some(dummy_bin));
    }
}
