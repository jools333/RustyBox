// Prevents console window on Windows
#![cfg_attr(windows, windows_subsystem = "windows")]

use std::path::PathBuf;

/// Per-user directory for application logs. Works for both normal and
/// elevated (Administrator) processes, unlike a mapped network drive.
fn log_dir() -> PathBuf {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("rustybox");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn main() {
    let dir = log_dir();
    let crash_dir = dir.clone();

    std::panic::set_hook(Box::new(move |info| {
        let msg = format!("PANIC: {}\n", info);
        let _ = std::fs::write(crash_dir.join("crash.log"), &msg);
    }));

    // On Windows, prompt for Administrator elevation via UAC on startup (similar to NekoBox)
    rustybox_lib::platform::ensure_elevation();

    let _ = std::fs::write(dir.join("startup.log"), "rustybox started\n");

    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("rustybox.log"));

    if let Ok(file) = log_file {
        let _ = tracing_subscriber::fmt()
            .with_writer(std::sync::Mutex::new(file))
            .with_env_filter(
                tracing_subscriber::EnvFilter::from_default_env()
                    .add_directive(tracing::Level::INFO.into()),
            )
            .try_init();
    } else {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::from_default_env()
                    .add_directive(tracing::Level::INFO.into()),
            )
            .try_init();
    }

    #[cfg(feature = "gui")]
    rustybox_lib::run();
}
