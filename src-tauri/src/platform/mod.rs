pub mod autostart;
pub mod hosts;

pub use hosts::{
    get_system_hosts_entries, get_system_hosts_path, parse_hosts_content, HostsEntries,
};

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

/// Opens native file picker dialog to select an `.ovpn` configuration file.
#[cfg(target_os = "windows")]
pub fn pick_ovpn_file_dialog() -> Option<std::path::PathBuf> {
    use std::os::windows::ffi::OsStringExt;

    #[repr(C)]
    #[allow(non_snake_case)]
    #[allow(clippy::upper_case_acronyms)]
    struct OPENFILENAMEW {
        lStructSize: u32,
        hwndOwner: isize,
        hInstance: isize,
        lpstrFilter: *const u16,
        lpstrCustomFilter: *mut u16,
        nMaxCustFilter: u32,
        nFilterIndex: u32,
        lpstrFile: *mut u16,
        nMaxFile: u32,
        lpstrFileTitle: *mut u16,
        nMaxFileTitle: u32,
        lpstrInitialDir: *const u16,
        lpstrTitle: *const u16,
        Flags: u32,
        nFileOffset: u16,
        nFileExtension: u16,
        lpstrDefExt: *const u16,
        lCustData: usize,
        lpfnHook: usize,
        lpTemplateName: *const u16,
        pvReserved: *mut std::ffi::c_void,
        dwReserved: u32,
        FlagsEx: u32,
    }

    #[link(name = "comdlg32")]
    extern "system" {
        fn GetOpenFileNameW(lpofn: *mut OPENFILENAMEW) -> i32;
    }

    let mut file_buf = vec![0u16; 1024];
    let mut filter: Vec<u16> = Vec::new();
    for part in [
        "OpenVPN Config (*.ovpn)",
        "*.ovpn",
        "All Files (*.*)",
        "*.*",
    ] {
        filter.extend(part.encode_utf16());
        filter.push(0);
    }
    filter.push(0);

    let title: Vec<u16> = "Select OpenVPN Configuration File\0"
        .encode_utf16()
        .collect();

    let mut ofn: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    ofn.lpstrFilter = filter.as_ptr();
    ofn.lpstrFile = file_buf.as_mut_ptr();
    ofn.nMaxFile = file_buf.len() as u32;
    ofn.lpstrTitle = title.as_ptr();
    // OFN_FILEMUSTEXIST (0x1000) | OFN_PATHMUSTEXIST (0x800) | OFN_EXPLORER (0x80000)
    ofn.Flags = 0x00001000 | 0x00000800 | 0x00080000;

    let res = unsafe { GetOpenFileNameW(&mut ofn) };
    if res != 0 {
        let len = file_buf
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(file_buf.len());
        let os_str = std::ffi::OsString::from_wide(&file_buf[..len]);
        Some(std::path::PathBuf::from(os_str))
    } else {
        None
    }
}

#[cfg(target_os = "linux")]
pub fn pick_ovpn_file_dialog() -> Option<std::path::PathBuf> {
    let mut cmd = std::process::Command::new("zenity");
    cmd.args([
        "--file-selection",
        "--file-filter=OpenVPN (*.ovpn) | *.ovpn",
        "--title=Select OpenVPN Configuration File",
    ]);
    cmd.stdin(std::process::Stdio::null());
    let output = cmd.output().ok()?;
    if output.status.success() {
        let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path_str.is_empty() {
            return Some(std::path::PathBuf::from(path_str));
        }
    }
    None
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
pub fn pick_ovpn_file_dialog() -> Option<std::path::PathBuf> {
    None
}
