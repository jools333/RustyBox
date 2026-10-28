//! Cross-platform autostart management.
//!
//! On Windows, applications requiring administrator elevation (such as RustyBox with TUN mode
//! via `requireAdministrator` manifest) CANNOT be launched by Windows Explorer from the standard
//! `Run` registry key at logon (Windows Explorer silently ignores/drops elevated items in `Run`
//! without prompting UAC).
//!
//! To allow elevated startup without UAC prompts or Explorer blocks, Windows autostart is managed
//! via the Windows Task Scheduler (`schtasks.exe`) with `HighestAvailable` run level at logon.
//!
//! On Linux / macOS, standard autostart (XDG autostart / LaunchAgent via `tauri-plugin-autostart`) is used.

use std::path::Path;

pub const TASK_NAME: &str = "RustyBox";

pub fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Generates UTF-16LE XML with BOM for Task Scheduler task.
///
/// schtasks /Create /XML strictly requires UTF-16LE with BOM (0xFF, 0xFE).
pub fn create_task_xml(exe_path: &Path, working_dir: Option<&Path>) -> Vec<u8> {
    let exe_str = xml_escape(&exe_path.to_string_lossy());
    let work_dir_str = working_dir
        .map(|p| {
            format!(
                "<WorkingDirectory>{}</WorkingDirectory>",
                xml_escape(&p.to_string_lossy())
            )
        })
        .unwrap_or_default();

    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>RustyBox elevated autostart at logon</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>true</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <IdleSettings>
      <StopOnIdleEnd>false</StopOnIdleEnd>
      <RestartOnIdle>false</RestartOnIdle>
    </IdleSettings>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <RunOnlyIfIdle>false</RunOnlyIfIdle>
    <WakeToRun>false</WakeToRun>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>7</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{}</Command>
      <Arguments>--autostart</Arguments>
      {}
    </Exec>
  </Actions>
</Task>"#,
        exe_str, work_dir_str
    );

    // Prepend UTF-16LE Byte Order Mark (0xFF, 0xFE)
    let mut bytes = vec![0xFF, 0xFE];
    for ch in xml.encode_utf16() {
        bytes.extend_from_slice(&ch.to_le_bytes());
    }
    bytes
}

#[cfg(target_os = "windows")]
pub mod windows {
    use super::*;
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x08000000;

    /// Clean legacy registry entries created by older versions or tauri-plugin-autostart
    pub fn clean_legacy_registry_entries() {
        let _ = Command::new("reg")
            .args([
                "delete",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                "/v",
                TASK_NAME,
                "/f",
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output();

        let _ = Command::new("reg")
            .args([
                "delete",
                r"HKLM\Software\Microsoft\Windows\CurrentVersion\Run",
                "/v",
                TASK_NAME,
                "/f",
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
    }

    pub fn enable() -> Result<(), String> {
        clean_legacy_registry_entries();

        let current_exe = std::env::current_exe()
            .map_err(|e| format!("Failed to get current executable path: {}", e))?;
        let clean_exe = crate::core::normalize_path(&current_exe);
        let working_dir = clean_exe.parent();

        // 1. Try XML creation first (configures battery and timeout settings)
        let temp_dir = std::env::temp_dir();
        let xml_path = temp_dir.join("rustybox_autostart_task.xml");
        let xml_bytes = create_task_xml(&clean_exe, working_dir);

        if std::fs::write(&xml_path, xml_bytes).is_ok() {
            let output = Command::new("schtasks")
                .args([
                    "/Create",
                    "/TN",
                    TASK_NAME,
                    "/XML",
                    &xml_path.to_string_lossy(),
                    "/F",
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .output();

            let _ = std::fs::remove_file(&xml_path);

            if let Ok(out) = output {
                if out.status.success() {
                    return Ok(());
                }
            }
        }

        // 2. Fallback to direct schtasks command line if XML fails
        let tr_arg = format!("\"{}\" --autostart", clean_exe.display());
        let output = Command::new("schtasks")
            .args([
                "/Create", "/TN", TASK_NAME, "/TR", &tr_arg, "/SC", "ONLOGON", "/RL", "HIGHEST",
                "/F",
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("Failed to execute schtasks: {}", e))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(format!("schtasks failed to create task: {}", err.trim()));
        }

        Ok(())
    }

    pub fn disable() -> Result<(), String> {
        clean_legacy_registry_entries();

        let _ = Command::new("schtasks")
            .args(["/Delete", "/TN", TASK_NAME, "/F"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();

        Ok(())
    }

    pub fn is_enabled() -> bool {
        if let Ok(output) = Command::new("schtasks")
            .args(["/Query", "/TN", TASK_NAME])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
        {
            if output.status.success() {
                return true;
            }
        }

        // Also check if legacy registry entry is still active
        if let Ok(output) = Command::new("reg")
            .args([
                "query",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                "/v",
                TASK_NAME,
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
        {
            if output.status.success() {
                return true;
            }
        }

        if let Ok(output) = Command::new("reg")
            .args([
                "query",
                r"HKLM\Software\Microsoft\Windows\CurrentVersion\Run",
                "/v",
                TASK_NAME,
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
        {
            if output.status.success() {
                return true;
            }
        }

        false
    }
}

#[cfg(feature = "gui")]
pub fn set_enabled(app: &tauri::AppHandle, enabled: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let _ = app;
        if enabled {
            windows::enable()
        } else {
            windows::disable()
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        use tauri_plugin_autostart::ManagerExt;
        let autolaunch = app.autolaunch();
        if enabled {
            autolaunch.enable().map_err(|e| e.to_string())
        } else {
            autolaunch.disable().map_err(|e| e.to_string())
        }
    }
}

#[cfg(feature = "gui")]
pub fn is_enabled(app: &tauri::AppHandle) -> Result<bool, String> {
    #[cfg(target_os = "windows")]
    {
        let _ = app;
        Ok(windows::is_enabled())
    }

    #[cfg(not(target_os = "windows"))]
    {
        use tauri_plugin_autostart::ManagerExt;
        app.autolaunch().is_enabled().map_err(|e| e.to_string())
    }
}

#[cfg(feature = "gui")]
pub fn sync_with_config(app: &tauri::AppHandle, should_be_enabled: bool) -> Result<(), String> {
    let currently_enabled = is_enabled(app).unwrap_or(false);
    if should_be_enabled != currently_enabled {
        set_enabled(app, should_be_enabled)
    } else {
        #[cfg(target_os = "windows")]
        {
            // On Windows, if autostart is already enabled, re-enabling ensures that if the binary
            // was updated or moved to a new path, the scheduled task XML is refreshed to the new path.
            if should_be_enabled {
                let _ = windows::enable();
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xml_escape() {
        assert_eq!(
            xml_escape("C:\\Users\\Bob & Alice <test> \"quotes\" 'single'"),
            "C:\\Users\\Bob &amp; Alice &lt;test&gt; &quot;quotes&quot; &apos;single&apos;"
        );
    }

    #[test]
    fn test_create_task_xml_structure_and_bom() {
        let exe = Path::new("C:\\Program Files\\RustyBox\\rustybox.exe");
        let work_dir = Path::new("C:\\Program Files\\RustyBox");
        let bytes = create_task_xml(exe, Some(work_dir));

        // Must start with UTF-16LE BOM
        assert!(bytes.len() > 2);
        assert_eq!(bytes[0], 0xFF);
        assert_eq!(bytes[1], 0xFE);

        // Decode UTF-16LE back to string to verify contents
        let u16_chars: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        let decoded = String::from_utf16(&u16_chars).expect("valid utf16");

        assert!(decoded.contains("<RunLevel>HighestAvailable</RunLevel>"));
        assert!(decoded.contains("<DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>"));
        assert!(decoded.contains("<StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>"));
        assert!(decoded.contains("<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>"));
        assert!(decoded.contains("<Arguments>--autostart</Arguments>"));
        assert!(decoded.contains("<Command>C:\\Program Files\\RustyBox\\rustybox.exe</Command>"));
        assert!(
            decoded.contains("<WorkingDirectory>C:\\Program Files\\RustyBox</WorkingDirectory>")
        );
    }
}
