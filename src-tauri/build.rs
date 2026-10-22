use std::path::PathBuf;

fn main() {
    if std::env::var("CARGO_FEATURE_GUI").is_ok() {
        let manifest_dir =
            PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into()));

        // Ensure the bin directory exists and contains at least a placeholder file
        // so that tauri-build's bundle glob "../bin/*" does not panic on a clean checkout.
        let bin_dir = manifest_dir.parent().unwrap_or(&manifest_dir).join("bin");
        let _ = std::fs::create_dir_all(&bin_dir);
        let placeholder = bin_dir.join("placeholder.txt");
        if !placeholder.exists() {
            let _ = std::fs::write(&placeholder, "RustyBox binaries directory\n");
        }

        // Ensure frontend/dist exists so tauri::generate_context!() does not panic
        // during clippy or cargo check if frontend hasn't been built yet.
        let frontend_dist = manifest_dir
            .parent()
            .unwrap_or(&manifest_dir)
            .join("frontend")
            .join("dist");
        let _ = std::fs::create_dir_all(&frontend_dist);
        let index_html = frontend_dist.join("index.html");
        if !index_html.exists() {
            let _ = std::fs::write(
                &index_html,
                "<!DOCTYPE html><html><body>RustyBox</body></html>\n",
            );
        }

        let tauri_conf = manifest_dir.join("tauri.conf.json");
        let ico_path = manifest_dir.join("icons").join("icon.ico");

        let manifest = r#"
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false" />
      </requestedPrivileges>
    </security>
  </trustInfo>
  <dependency>
    <dependentAssembly>
      <assemblyIdentity
        type="win32"
        name="Microsoft.Windows.Common-Controls"
        version="6.0.0.0"
        processorArchitecture="*"
        publicKeyToken="6595b64144ccf1df"
        language="*"
      />
    </dependentAssembly>
  </dependency>
</assembly>
"#;

        let win_attrs = tauri_build::WindowsAttributes::new()
            .window_icon_path(ico_path)
            .app_manifest(manifest);
        let attrs = tauri_build::Attributes::new()
            .config_path(tauri_conf)
            .windows_attributes(win_attrs);

        tauri_build::try_build(attrs).expect("failed to run tauri-build");
    }
}
