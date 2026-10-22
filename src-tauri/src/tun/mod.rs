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
        let mut is_elevated = check_elevation();

        #[cfg(not(target_os = "windows"))]
        {
            let _ = singbox_binary_path;
        }

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
                if let Ok(current_exe) = std::env::current_exe() {
                    if let Some(parent) = current_exe.parent() {
                        let candidates = [
                            parent.join("wintun.dll"),
                            parent.join("bin").join("wintun.dll"),
                            parent.join("resources").join("wintun.dll"),
                            parent.join("resources").join("bin").join("wintun.dll"),
                            parent.join("_up_").join("bin").join("wintun.dll"),
                            parent.join("_up_").join("wintun.dll"),
                        ];
                        for candidate in candidates {
                            if candidate.exists() {
                                driver_found = true;
                                details
                                    .push(format!("Found wintun.dll at {}", candidate.display()));
                                break;
                            }
                        }
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
                if let Some(bin_path) = singbox_binary_path {
                    if check_binary_cap_net_admin(bin_path) {
                        is_elevated = true;
                        details.push(format!(
                            "sing-box binary has CAP_NET_ADMIN capability: {}",
                            bin_path.display()
                        ));
                    }
                } else {
                    let candidates = [
                        Path::new("/usr/lib/RustyBox/bin/sing-box"),
                        Path::new("/usr/lib/rustybox/bin/sing-box"),
                    ];
                    for candidate in candidates {
                        if candidate.exists() && check_binary_cap_net_admin(candidate) {
                            is_elevated = true;
                            details.push(format!(
                                "Found sing-box with CAP_NET_ADMIN capability: {}",
                                candidate.display()
                            ));
                            break;
                        }
                    }
                }
            }

            if is_elevated {
                details.push(
                    "Elevation check passed (root or CAP_NET_ADMIN capability detected)."
                        .to_string(),
                );
            } else {
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

    /// Removes any orphaned or disconnected Wintun / sing-tun network adapters
    /// and conflicting IP addresses left over from an abrupt termination or crash.
    pub fn cleanup_stale_adapter(&self, interface_name: &str) {
        let _ = interface_name;
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;

            // 1. Remove conflicting IP address 172.19.0.1 if bound to our leftover adapter
            let _ = std::process::Command::new("powershell")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    &format!(
                        "Get-NetIPAddress -IPAddress 172.19.0.1 -ErrorAction SilentlyContinue | Where-Object {{ $_.InterfaceAlias -eq '{}' }} | Remove-NetIPAddress -Confirm:$false -ErrorAction SilentlyContinue",
                        interface_name
                    ),
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .status();

            // 2. Scan pnputil for DISCONNECTED/orphaned Wintun devices and remove them.
            // NEVER remove working/active adapters (which might belong to active VPNs or current session).
            if let Ok(output) = std::process::Command::new("pnputil")
                .args(["/enum-devices", "/class", "Net"])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
            {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stale_instances = parse_disconnected_wintun_instances(&stdout);
                let removed_any = !stale_instances.is_empty();
                for instance in stale_instances {
                    let _ = std::process::Command::new("pnputil")
                        .args(["/remove-device", &instance])
                        .creation_flags(CREATE_NO_WINDOW)
                        .status();
                }
                // If any device was removed, give Windows PnP manager a brief moment to settle
                if removed_any {
                    std::thread::sleep(std::time::Duration::from_millis(300));
                }
            }
        }
    }
}

/// Parses the output of `pnputil /enum-devices /class Net` to find instance IDs
/// of all Wintun / sing-tun devices so they can be cleaned up before a new connection.
pub fn parse_all_wintun_instances(pnputil_output: &str) -> Vec<String> {
    let mut instances = Vec::new();
    let mut current_instance: Option<String> = None;
    let mut is_wintun = false;

    for line in pnputil_output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if let Some(id) = current_instance.take() {
                if is_wintun {
                    instances.push(id);
                }
            }
            current_instance = None;
            is_wintun = false;
            continue;
        }

        let lower = trimmed.to_lowercase();
        if let Some(idx) = trimmed.find(r"SWD\Wintun\") {
            let id = trimmed[idx..].trim().to_string();
            current_instance = Some(id);
            is_wintun = true;
        } else if lower.contains("sing-tun") || lower.contains("wintun") {
            is_wintun = true;
        }
    }

    if let Some(id) = current_instance {
        if is_wintun {
            instances.push(id);
        }
    }

    instances
}

/// Parses the output of `pnputil /enum-devices /class Net` to find instance IDs
/// of disconnected or stale Wintun/sing-tun devices.
pub fn parse_disconnected_wintun_instances(pnputil_output: &str) -> Vec<String> {
    let mut instances = Vec::new();
    let mut current_instance: Option<String> = None;
    let mut is_wintun = false;
    let mut is_disconnected = false;

    for line in pnputil_output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if let Some(id) = current_instance.take() {
                if is_wintun && is_disconnected {
                    instances.push(id);
                }
            }
            current_instance = None;
            is_wintun = false;
            is_disconnected = false;
            continue;
        }

        let lower = trimmed.to_lowercase();
        // Check for instance ID line (contains SWD\Wintun\)
        if let Some(idx) = trimmed.find(r"SWD\Wintun\") {
            let id = trimmed[idx..].trim().to_string();
            current_instance = Some(id);
            is_wintun = true;
        } else if lower.contains("sing-tun") || lower.contains("wintun") {
            is_wintun = true;
        }

        // Status line: "Disconnected", "Отключено", or error
        if lower.contains("отключено")
            || lower.contains("disconnected")
            || lower.contains("disabled")
            || lower.contains("не работает")
            || lower.contains("error")
        {
            is_disconnected = true;
        }
    }

    if let Some(id) = current_instance {
        if is_wintun && is_disconnected {
            instances.push(id);
        }
    }

    instances
}

pub fn check_elevation() -> bool {
    #[cfg(target_os = "windows")]
    {
        extern "system" {
            fn GetCurrentProcess() -> isize;
            fn OpenProcessToken(
                process: isize,
                desired_access: u32,
                token_handle: *mut isize,
            ) -> i32;
            fn GetTokenInformation(
                token_handle: isize,
                token_information_class: u32,
                token_information: *mut std::ffi::c_void,
                token_information_length: u32,
                return_length: *mut u32,
            ) -> i32;
            fn CloseHandle(handle: isize) -> i32;
        }

        #[repr(C)]
        struct TokenElevation {
            token_is_elevated: u32,
        }

        unsafe {
            let mut token: isize = 0;
            // TOKEN_QUERY = 0x0008
            if OpenProcessToken(GetCurrentProcess(), 0x0008, &mut token) != 0 {
                let mut elevation = TokenElevation {
                    token_is_elevated: 0,
                };
                let mut ret_len = 0;
                // TokenElevation = 20
                let res = GetTokenInformation(
                    token,
                    20,
                    &mut elevation as *mut _ as *mut std::ffi::c_void,
                    std::mem::size_of::<TokenElevation>() as u32,
                    &mut ret_len,
                );
                CloseHandle(token);
                res != 0 && elevation.token_is_elevated != 0
            } else {
                false
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        // On Linux, EUID == 0 means root, or current process has CAP_NET_ADMIN
        libc_or_zero() == 0 || current_process_has_cap_net_admin()
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

#[cfg(target_os = "linux")]
fn current_process_has_cap_net_admin() -> bool {
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if let Some(stripped) = line.strip_prefix("CapEff:\t") {
                if let Ok(val) = u64::from_str_radix(stripped.trim(), 16) {
                    if (val & (1 << 12)) != 0 {
                        return true;
                    }
                }
            }
        }
    }
    false
}

#[cfg(target_os = "linux")]
pub fn check_binary_cap_net_admin(binary_path: &Path) -> bool {
    // 1. Try running getcap tool
    let getcap_candidates = ["getcap", "/usr/sbin/getcap", "/sbin/getcap"];
    for getcap in &getcap_candidates {
        if let Ok(output) = std::process::Command::new(getcap).arg(binary_path).output() {
            if output.status.success() {
                let out = String::from_utf8_lossy(&output.stdout);
                if out.contains("cap_net_admin") {
                    return true;
                }
            }
        }
    }

    // 2. Direct xattr fallback: inspect security.capability via libc getxattr
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    extern "C" {
        fn getxattr(
            path: *const std::os::raw::c_char,
            name: *const std::os::raw::c_char,
            value: *mut std::os::raw::c_void,
            size: usize,
        ) -> isize;
    }

    if let Ok(c_path) = CString::new(binary_path.as_os_str().as_bytes()) {
        if let Ok(attr) = CString::new("security.capability") {
            let mut buf = [0u8; 64];
            let res = unsafe {
                getxattr(
                    c_path.as_ptr(),
                    attr.as_ptr(),
                    buf.as_mut_ptr() as *mut _,
                    buf.len(),
                )
            };
            if res >= 8 {
                // Byte 4..8 contains permitted capability bitmask.
                // CAP_NET_ADMIN is capability bit 12 (1 << 12 = 0x1000).
                let permitted = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
                if (permitted & (1 << 12)) != 0 {
                    return true;
                }
            }
        }
    }

    false
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

    #[test]
    fn test_parse_disconnected_wintun_instances() {
        let sample = r#"
Instance ID:                SWD\Wintun\{5AB05583-B50F-8579-BBD8-7243AE1FBEF5}
Device Description:         sing-tun Tunnel
Class Name:                 Net
Status:                     Отключено

Instance ID:                SWD\Wintun\{1BF0EB02-8C39-B209-F8B5-8B00F51DB18B}
Device Description:         sing-tun Tunnel
Class Name:                 Net
Status:                     Работает

Instance ID:                ROOT\NET\0000
Device Description:         Other Adapter
Class Name:                 Net
Status:                     Disconnected
"#;
        let stale = parse_disconnected_wintun_instances(sample);
        assert_eq!(stale.len(), 1);
        assert_eq!(
            stale[0],
            r"SWD\Wintun\{5AB05583-B50F-8579-BBD8-7243AE1FBEF5}"
        );
    }

    #[test]
    fn test_parse_all_wintun_instances() {
        let sample = r#"
Instance ID:                SWD\Wintun\{5AB05583-B50F-8579-BBD8-7243AE1FBEF5}
Device Description:         sing-tun Tunnel
Class Name:                 Net
Status:                     Отключено

Instance ID:                SWD\Wintun\{1BF0EB02-8C39-B209-F8B5-8B00F51DB18B}
Device Description:         sing-tun Tunnel
Class Name:                 Net
Status:                     Работает

Instance ID:                ROOT\NET\0000
Device Description:         Other Adapter
Class Name:                 Net
Status:                     Disconnected
"#;
        let all = parse_all_wintun_instances(sample);
        assert_eq!(all.len(), 2);
        assert_eq!(all[0], r"SWD\Wintun\{5AB05583-B50F-8579-BBD8-7243AE1FBEF5}");
        assert_eq!(all[1], r"SWD\Wintun\{1BF0EB02-8C39-B209-F8B5-8B00F51DB18B}");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_linux_capabilities_prerequisites() {
        let mgr = TunManager::new();
        let target = Path::new("/usr/lib/RustyBox/bin/sing-box");
        if target.exists() {
            let has_cap = check_binary_cap_net_admin(target);
            assert!(
                has_cap,
                "Installed /usr/lib/RustyBox/bin/sing-box should have CAP_NET_ADMIN"
            );
            let prereqs = mgr.check_prerequisites(Some(target));
            assert!(
                prereqs.is_elevated,
                "Prerequisites must report is_elevated=true when binary has capability"
            );
        }
    }
}
