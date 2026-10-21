use std::path::PathBuf;

fn main() {
    if std::env::var("CARGO_FEATURE_GUI").is_ok() {
        let manifest_dir =
            PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into()));
        let tauri_conf = manifest_dir.join("tauri.conf.json");
        let ico_path = manifest_dir.join("icons").join("icon.ico");

        let win_attrs = tauri_build::WindowsAttributes::new().window_icon_path(ico_path);
        let attrs = tauri_build::Attributes::new()
            .config_path(tauri_conf)
            .windows_attributes(win_attrs);

        tauri_build::try_build(attrs).expect("failed to run tauri-build");
    }
}
