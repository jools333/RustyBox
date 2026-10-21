use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunPrerequisites {
    pub is_supported: bool,
    pub has_driver: bool,
    pub is_elevated: bool,
    pub details: Vec<String>,
}

#[derive(Clone, Default)]
pub struct TunManager;

impl TunManager {
    pub fn new() -> Self {
        Self
    }

    pub fn check_prerequisites(&self, singbox_binary_path: Option<&Path>) -> TunPrerequisites {
        let mut details = Vec::new();
        let mut has_driver = false;
        let is_elevated = check_elevation();

        #[cfg(target_os = "windows")]
        {
            // On Windows, wintun.dll is required next to sing-box or in System32
            let mut driver_found = false;
            if let Some(bin_path) = singbox_binary_path {
                if let Some(parent) = bin_path.parent() {
                    let wintun = parent.join("wintun.dll");
                    if wintun.exists() {
                        driver_found = true;
                        details.push(format!("Found wintun.dll at {}", wintun.display()));
                    }
                }
            }

            if !driver_found {
                // Check current working dir and ./bin
                if Path::new("wintun.dll").exists() {
                    driver_found = true;
                    details.push("Found wintun.dll in current directory".to_string());
                } else if Path::new("bin/wintun.dll").exists() {
                    driver_found = true;
                    details.push("Found wintun.dll in bin directory".to_string());
                } else if Path::new("C:\\Windows\\System32\\wintun.dll").exists() {
                    driver_found = true;
                    details.push("Found wintun.dll in System32".to_string());
                }
            }

            if driver_found {
                has_driver = true;
            } else {
                details.push(
                    "wintun.dll not found. Please place wintun.dll alongside sing-box.exe"
                        .to_string(),
                );
            }

            if !is_elevated {
                details.push(
                    "Administrator privileges are recommended/required for TUN mode on Windows."
                        .to_string(),
                );
            }
        }

        #[cfg(target_os = "linux")]
        {
            let tun_dev = Path::new("/dev/net/tun");
            if tun_dev.exists() {
                has_driver = true;
                details.push("Found /dev/net/tun device".to_string());
            } else {
                details.push("/dev/net/tun device not found. Ensure tun kernel module is loaded: modprobe tun".to_string());
            }

            if !is_elevated {
                details.push("Root privileges or CAP_NET_ADMIN capability required for TUN network operations.".to_string());
            }
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            details.push("Operating system is not Windows or Linux".to_string());
        }

        TunPrerequisites {
            is_supported: true,
            has_driver,
            is_elevated,
            details,
        }
    }

    /// Generates sing-box JSON inbound block for TUN
    pub fn generate_tun_inbound(&self, interface_name: &str) -> serde_json::Value {
        serde_json::json!({
            "type": "tun",
            "tag": "tun-in",
            "interface_name": interface_name,
            "address": [
                "172.19.0.1/30",
                "fdfe:dcba:9876::1/126"
            ],
            "auto_route": true,
            "strict_route": true,
            "stack": "mixed"
        })
    }
}

pub fn check_elevation() -> bool {
    #[cfg(target_os = "windows")]
    {
        // Safe elevation check on Windows
        use std::process::Command;
        if let Ok(output) = Command::new("net").arg("session").output() {
            output.status.success()
        } else {
            false
        }
    }

    #[cfg(target_os = "linux")]
    {
        // On Linux, EUID == 0 means root
        unsafe { libc_or_zero() == 0 }
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        false
    }
}

#[cfg(target_os = "linux")]
fn libc_or_zero() -> u32 {
    // Check EUID using std or libc
    std::process::Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_tun_inbound() {
        let mgr = TunManager::new();
        let inbound = mgr.generate_tun_inbound("rustybox-tun");
        assert_eq!(inbound["type"], "tun");
        assert_eq!(inbound["interface_name"], "rustybox-tun");
        assert_eq!(inbound["auto_route"], true);
        assert_eq!(inbound["strict_route"], true);
        assert_eq!(inbound["stack"], "mixed");
        assert!(inbound["address"].is_array());
    }
}
