// Prevents console window on Windows
#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() {
    std::panic::set_hook(Box::new(|info| {
        let msg = format!("PANIC: {}\n", info);
        let _ = std::fs::write("z:\\jvpn\\crash.log", &msg);
        let _ = std::fs::write("C:\\Users\\lotarev\\rustybox_crash.log", &msg);
    }));

    let _ = std::fs::write("z:\\jvpn\\startup.log", "rustybox started\n");

    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("z:\\jvpn\\rustybox.log");

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
