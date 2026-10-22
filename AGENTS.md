# AGENTS.md — Development & Operational Guide for RustyBox

This document serves as the primary technical specification, operational reference, and coding handbook for AI agents and human developers contributing to **RustyBox**.

---

## 1. Project Overview & Core Mission

**RustyBox** is an open-source, lightweight, cross-platform desktop VPN client.
- **Backend**: Rust 2021 edition + **Tauri 2**.
- **Frontend**: React 18 + TypeScript + Vite + Tailwind CSS + Lucide Icons.
- **Core Engine**: [sing-box](https://github.com/SagerNet/sing-box) (v1.14+) managed as an independent external child process.
- **License**: MIT (client code); `sing-box` is GPLv3-licensed and kept isolated via clean process boundaries (command-line, pipes, and JSON configuration).

### Supported Protocols
- **VLESS**: REALITY with `x25519`, TLS, UUID v4 authentication, `xtls-rprx-vision` flow.
- **Shadowsocks**: Standard AEAD (`aes-128-gcm`, `aes-256-gcm`, `chacha20-ietf-poly1305`) and AEAD 2022 ciphers (`2022-blake3-aes-128-gcm`, `2022-blake3-aes-256-gcm`, `2022-blake3-chacha20-poly1305`).

### Operating Modes
- **TUN Virtual Interface Mode**: Complete system-wide packet capture via `rustybox-tun` adapter (`172.19.0.1/30` IPv4, `fdfe:dcba:9876::1/126` IPv6).
- **Domain Routing Engine**: Direct vs. VPN policy rules, default fallback action, protocol sniffing (`action: "sniff"`), and full DNS hijacking (`port: 53, action: "hijack-dns"`).

---

## 2. Repository Layout & Architecture

```
z:\jvpn\
├── bin\                      # Core binaries and platform drivers
│   ├── sing-box.exe          # Official sing-box v1.14.3 executable
│   └── wintun.dll            # Wintun TUN driver for Windows (v0.14+)
├── src-tauri\                # Rust / Tauri 2 backend crate
│   ├── Cargo.toml            # Dependencies, profiles, and features
│   ├── build.rs              # Resource compilation and Windows icon handling
│   ├── tauri.conf.json       # Window geometry, title, tray, and asset dist mapping
│   ├── icons\                # Multi-resolution application & tray icons
│   ├── src\
│   │   ├── main.rs           # GUI entry point with #![cfg_attr(windows, windows_subsystem = "windows")]
│   │   ├── lib.rs            # Application builder, lifecycle hooks, IPC command registration
│   │   ├── app.rs            # AppState container: holds managers for config, profiles, rules, core
│   │   ├── commands\         # Tauri IPC command handlers
│   │   ├── config\           # AppConfig storage (JSON persistence, corrupted file backup)
│   │   ├── core\             # sing-box 1.14 JSON config generator & child process supervisor
│   │   ├── logging\          # Ring buffer (1000 lines), secret masking, diagnostic export
│   │   ├── platform\         # Native Win32 elevation checks & platform diagnostic notes
│   │   ├── profiles\         # URI parser and validator for vless:// and ss:// links
│   │   ├── routing\          # Domain rule engine (match types, priority sorting, fallback)
│   │   ├── tray\             # System tray menu and window visibility toggle
│   │   ├── tun\              # TUN inbound generator and Wintun driver verification
│   │   └── vpn\              # Lifecycle state machine (Disconnected, Starting, Connected, Stopping, Error)
│   └── tests\
│       └── integration_tests.rs # 12 core test scenarios validating all requirements
├── frontend\                 # React + TypeScript + Vite UI
│   ├── package.json          # Dependencies: @tauri-apps/api, react, lucide-react, tailwindcss
│   ├── vite.config.ts        # Vite configuration (base: "./" for reliable relative asset paths)
│   ├── index.html            # HTML entry point with relative assets
│   ├── src\
│   │   ├── types.ts          # TypeScript interfaces matching backend Rust structs
│   │   ├── api.ts            # Dynamic Tauri IPC wrapper (never caches isTauri too early)
│   │   ├── App.tsx           # Tab routing, state polling, active notifications
│   │   └── components\       # Header, ConnectionPanel, ProfilesView, RoutingView, SettingsView, LogsView
│   └── dist\                 # Production web bundle embedded into the Rust executable
├── scripts\                  # Cross-platform build & download scripts
│   ├── build-windows.ps1     # Windows build script
│   ├── build-linux.sh        # Linux build script
│   ├── download-singbox.ps1  # Automated sing-box & wintun downloader (Windows)
│   └── download-singbox.sh   # Automated sing-box downloader (Linux)
├── .github\workflows\
│   └── ci.yml                # CI/CD: fmt check, clippy (-D warnings), tests, and builds
├── README.md                 # User-facing and build documentation
├── AGENTS.md                 # This file
└── LICENSE                   # MIT License with sing-box GPLv3 compliance notice
```

---

## 3. Environment & Toolchain Constraints

### Windows GNU Toolchain & `w64devkit`
- The active Rust toolchain in this environment is `stable-x86_64-pc-windows-gnu`.
- MinGW/GCC toolchain binaries (`gcc`, `windres`, `ar`, `dlltool`, `as`) are located in `C:\Users\lotarev\.w64devkit\bin`.
- **Rule for Agents**: Whenever executing `cargo` commands in PowerShell, ALWAYS prepend the path:
  ```powershell
  $env:PATH = "C:\Users\lotarev\.w64devkit\bin;$env:PATH"
  ```

### WSL Mapped Drive (`Z:\`) File Locking
- `Z:\` is a network 9P mount (`\\wsl.localhost\Ubuntu\...`).
- Incremental compilation on Windows GNU over 9P mounts causes OS error `-2147024895` due to file locking issues.
- **Rule for Agents**: Always disable incremental compilation before running cargo:
  ```powershell
  $env:CARGO_INCREMENTAL = "0"
  ```

### Decoupled GUI Feature for Fast Testing
- `src-tauri/Cargo.toml` provides `[features] default = ["gui"]` with `tauri = { optional = true }`.
- Testing headless logic (profiles, URI parsing, routing, config generation, secret masking, VPN lifecycle) should run without GUI dependencies:
  ```powershell
  cargo test --no-default-features
  ```
- This avoids missing DLL entrypoint issues on headless test runners while ensuring 100% test coverage.

### Windows Subsystem (No Console Window)
- To prevent a secondary command console window (`cmd`/`conhost`) from appearing on Windows, `src-tauri/src/main.rs` uses:
  ```rust
  #![cfg_attr(windows, windows_subsystem = "windows")]
  ```
- The PE header is compiled with `IMAGE_SUBSYSTEM_WINDOWS_GUI` (`Subsystem: 2`). Do not revert to `not(debug_assertions)` unless specifically requested.

---

## 4. sing-box 1.14 Configuration Rules

When modifying [src-tauri/src/core/mod.rs](file:///z:/jvpn/src-tauri/src/core/mod.rs), strictly adhere to the sing-box 1.14 configuration syntax:

1. **DNS Block Syntax**:
   - Every DNS server must have an explicit `type`:
     ```json
     {
       "tag": "dns-remote",
       "type": "udp",
       "server": "1.1.1.1",
       "detour": "proxy"
     }
     ```
   - **`dns-direct` requirement**:
     DO NOT use `"detour": "direct"` on a `"type": "local"` DNS server. In sing-box 1.14, `detour` on `local` causes:
     `FATAL: detour to an empty direct outbound makes no sense`.
     Instead, specify an external UDP server for direct queries:
     ```json
     {
       "tag": "dns-direct",
       "type": "udp",
       "server": "77.88.8.8"
     }
     ```
   - Use `route.default_domain_resolver: "dns-direct"`.

2. **DNS Interception**:
   - In `route.rules`, always include both:
     ```json
     { "action": "sniff" },
     { "port": 53, "action": "hijack-dns" },
     { "protocol": "dns", "action": "hijack-dns" }
     ```
   - `{ "port": 53, "action": "hijack-dns" }` intercepts raw UDP DNS queries immediately, preventing DNS leaks or drops before protocol sniffing completes.

3. **Proxy Server Bypass**:
   - To prevent routing loops, the domain of the active proxy server itself (`profile.server`) must be routed directly:
     ```json
     { "domain": [profile.server], "outbound": "direct" }
     ```
   - And resolved through direct DNS:
     ```json
     { "domain": [profile.server], "server": "dns-direct" }
     ```

4. **TUN Inbound Format**:
   - Address must be an array: `["172.19.0.1/30", "fdfe:dcba:9876::1/126"]`.
   - `auto_route: true`.
   - `strict_route: true`.
   - `stack: "mixed"`.

---

## 5. Security & Sanitization Rules

1. **Secret Masking**:
   - All UUIDs (`[0-9a-fA-F]{8}-[0-9a-fA-F]{4}...`), passwords, REALITY public/private keys, and base64 credentials must pass through `mask_secrets` in [src-tauri/src/logging/mod.rs](file:///z:/jvpn/src-tauri/src/logging/mod.rs) before:
     - Storing in the in-memory log buffer (`LogManager`).
     - Writing to the on-disk log file (repo-local `z:\jvpn\singbox.log` during development; `%APPDATA%\rustybox\singbox.log` when the repo drive is unavailable, e.g. in an elevated process).
     - Sending to the frontend via Tauri IPC (`get_logs`, `get_diagnostic_report`).
2. **Configuration Protection**:
   - User configs are saved to the OS app data directory (`%APPDATA%/rustybox/config.json` on Windows).
   - If a config file is corrupted, the application MUST back it up as `config.json.corrupted.<timestamp>` and initialize safe defaults rather than crashing.
3. **Core Supervision**:
   - `start_core` MUST NOT report success before sing-box has logged `sing-box started` and installed the TUN interface/routes.
   - If the core exits unexpectedly, the connection state MUST transition to `Error` (`ConnectionManager::get_status` checks `CoreManager::is_running`) instead of displaying `Connected` while traffic silently bypasses the VPN.

---

## 6. Frontend Guidelines

1. **Relative Asset Paths**:
   - In `frontend/vite.config.ts`, `base: "./"` is mandatory.
   - This ensures WebView2 correctly resolves embedded CSS, JS, and image assets regardless of the host origin protocol (`tauri://` or `http://tauri.localhost`).
2. **Dynamic IPC Initialization**:
   - In `frontend/src/api.ts`, NEVER use `const isTauri = ...` evaluated at module import time.
   - Always evaluate dynamically:
     ```typescript
     const isTauri = (): boolean =>
       typeof window !== "undefined" &&
       ("__TAURI_INTERNALS__" in window || "__TAURI__" in window);
     ```

---

## 7. Auto-Update Architecture & Release Signing

1. **Updater Mechanism**:
   - RustyBox uses `@tauri-apps/plugin-updater` and `tauri-plugin-updater` targeting GitHub Releases (`https://github.com/jools333/RustyBox/releases/latest/download/latest.json`).
   - Releases are signed with Minisign/Ed25519.
   - The public key is configured in `src-tauri/tauri.conf.json`.
   - The private key is provided as a secret (`TAURI_SIGNING_PRIVATE_KEY`) in GitHub Actions CI/CD.
2. **Process Safety on Windows**:
   - Before downloading or applying an in-app update, RustyBox disconnects any active VPN session (`ConnectionManager::disconnect`).
   - This ensures `sing-box.exe` has terminated and `wintun.dll` / TUN interface handles are closed, avoiding `ERROR_ACCESS_DENIED` / "File In Use" errors when the installer replaces files.

---

## 8. Developer & Agent Cheat Sheet

### Running Tests
```powershell
$env:PATH = "C:\Users\lotarev\.w64devkit\bin;$env:PATH"
$env:CARGO_INCREMENTAL = "0"
cargo test --no-default-features
```

### Checking Code Formatting & Clippy
```powershell
$env:PATH = "C:\Users\lotarev\.w64devkit\bin;$env:PATH"
cargo fmt --all -- --check
cargo clippy --package rustybox --bin rustybox -- -D warnings
```

### Building the Desktop Executable
```powershell
# 1. Build frontend
wsl -d Ubuntu -- bash -c "cd /root/projects/jvpn/frontend && npm run build"

# 2. Build Rust executable
$env:PATH = "C:\Users\lotarev\.w64devkit\bin;$env:PATH"
$env:CARGO_INCREMENTAL = "0"
cargo build --package rustybox --bin rustybox

# 3. Ensure bin dependencies are alongside binary
Copy-Item "z:\jvpn\bin\sing-box.exe" "z:\jvpn\target\debug\sing-box.exe" -Force
Copy-Item "z:\jvpn\bin\wintun.dll" "z:\jvpn\target\debug\wintun.dll" -Force
```

### Running the Application with Elevation (TUN Mode)
On Windows, virtual TUN adapters and routing table changes require Administrator rights:
1. Open PowerShell or Command Prompt as **Administrator**.
2. Run:
   ```powershell
   cd z:\jvpn
   .\target\debug\rustybox.exe
   ```
3. Logs will stream to `%APPDATA%\rustybox\singbox.log` (when running elevated) or to `z:\jvpn\singbox.log` (when the mapped drive is available). Application diagnostics are written to `%APPDATA%\rustybox\`.
