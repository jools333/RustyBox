pub mod app;
pub mod config;
pub mod core;
pub mod logging;
pub mod platform;
pub mod profiles;
pub mod routing;
pub mod tun;
pub mod vpn;

#[cfg(feature = "gui")]
pub mod commands;
#[cfg(feature = "gui")]
pub mod tray;

#[cfg(feature = "gui")]
use app::AppState;
#[cfg(feature = "gui")]
use tauri::{Manager, WindowEvent};

#[cfg(feature = "gui")]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = AppState::new();

    tauri::Builder::default()
        .manage(app_state.clone())
        .setup(move |app| {
            if let Err(e) = tray::setup_tray(app.handle()) {
                tracing::warn!("Failed to setup system tray: {}", e);
            }

            // Handle window close event (minimize to tray if configured)
            if let Some(main_window) = app.get_webview_window("main") {
                let state_clone = app_state.clone();
                let win_clone = main_window.clone();
                main_window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        let cfg = state_clone.config.get_config();
                        if cfg.close_to_tray {
                            api.prevent_close();
                            let _ = win_clone.hide();
                        }
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::connect_vpn,
            commands::disconnect_vpn,
            commands::get_profiles,
            commands::add_profile,
            commands::import_profile_uri,
            commands::update_profile,
            commands::delete_profile,
            commands::select_profile,
            commands::get_routing_rules,
            commands::save_routing_rules,
            commands::add_routing_rule,
            commands::update_routing_rule,
            commands::delete_routing_rule,
            commands::get_config,
            commands::save_config,
            commands::get_logs,
            commands::clear_logs,
            commands::get_diagnostic_report,
            commands::get_core_status,
            commands::check_tun_prerequisites,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                // Ensure core is killed when app exits
                if let Some(state) = app_handle.try_state::<AppState>() {
                    let vpn = state.vpn.clone();
                    tauri::async_runtime::block_on(async move {
                        let _ = vpn.disconnect().await;
                    });
                }
            }
        });
}
