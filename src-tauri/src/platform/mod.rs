pub mod autostart;

pub fn get_os_info() -> String {
    #[cfg(target_os = "windows")]
    {
        "Windows (NT)".to_string()
    }
    #[cfg(target_os = "linux")]
    {
        "Linux".to_string()
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        "Unknown OS".to_string()
    }
}

pub fn get_platform_tun_notes() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "On Windows, TUN mode requires administrator privileges and the Wintun driver (wintun.dll) placed alongside sing-box.exe."
    }
    #[cfg(target_os = "linux")]
    {
        "On Linux, TUN mode requires /dev/net/tun and either root privileges or the CAP_NET_ADMIN Linux capability on the binary."
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        "Unsupported platform for TUN driver."
    }
}

/// Relaunches the current process with Administrator privileges on Windows using ShellExecuteW "runas".
#[cfg(target_os = "windows")]
pub fn relaunch_elevated() -> Result<(), String> {
    let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let args: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| a != "--no-elevation")
        .collect();
    let params = args
        .iter()
        .map(|a| {
            if a.contains(' ') {
                format!("\"{}\"", a)
            } else {
                a.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    use std::os::windows::ffi::OsStrExt;
    let exe_wide: Vec<u16> = current_exe
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let params_wide: Vec<u16> = std::ffi::OsStr::new(&params)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let verb_wide: Vec<u16> = std::ffi::OsStr::new("runas")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    #[link(name = "shell32")]
    extern "system" {
        fn ShellExecuteW(
            hwnd: isize,
            lp_operation: *const u16,
            lp_file: *const u16,
            lp_parameters: *const u16,
            lp_directory: *const u16,
            n_show_cmd: i32,
        ) -> isize;
    }

    // SW_SHOWNORMAL = 1
    let ret = unsafe {
        ShellExecuteW(
            0,
            verb_wide.as_ptr(),
            exe_wide.as_ptr(),
            if params.is_empty() {
                std::ptr::null()
            } else {
                params_wide.as_ptr()
            },
            std::ptr::null(),
            1,
        )
    };

    if ret as usize > 32 {
        std::process::exit(0);
    } else {
        Err(format!(
            "UAC elevation request was declined or failed (code {})",
            ret
        ))
    }
}

/// No-op on non-Windows platforms.
#[cfg(not(target_os = "windows"))]
pub fn relaunch_elevated() -> Result<(), String> {
    Err("Relaunch as administrator is only supported on Windows".to_string())
}

/// Ensures the application runs with Administrator privileges on Windows (UAC prompt),
/// similar to NekoBox behavior. If user declines or passes `--no-elevation`, it continues.
pub fn ensure_elevation() {
    #[cfg(target_os = "windows")]
    {
        if std::env::args().any(|a| a == "--no-elevation") {
            return;
        }

        if !crate::tun::check_elevation() {
            let _ = relaunch_elevated();
        }
    }
}
