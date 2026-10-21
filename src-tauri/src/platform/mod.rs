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
