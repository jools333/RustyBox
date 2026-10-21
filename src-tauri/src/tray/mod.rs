use crate::app::AppState;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let show_item = MenuItemBuilder::with_id("show_window", "Show RustyBox").build(app)?;
    let connect_item =
        MenuItemBuilder::with_id("toggle_connect", "Connect / Disconnect").build(app)?;
    let settings_item = MenuItemBuilder::with_id("open_settings", "Settings").build(app)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItemBuilder::with_id("quit", "Exit").build(app)?;

    let menu = MenuBuilder::new(app)
        .items(&[
            &show_item,
            &connect_item,
            &settings_item,
            &separator,
            &quit_item,
        ])
        .build()?;

    let icon = match app.default_window_icon() {
        Some(icon) => icon.clone(),
        None => {
            #[cfg(windows)]
            {
                if let Ok(icon) = tauri::image::Image::from_app_icon_resource(32) {
                    icon
                } else {
                    let bytes = include_bytes!("../../icons/32x32.png");
                    tauri::image::Image::from_bytes(bytes)?
                }
            }
            #[cfg(not(windows))]
            {
                let bytes = include_bytes!("../../icons/32x32.png");
                tauri::image::Image::from_bytes(bytes)?
            }
        }
    };

    let _tray = TrayIconBuilder::with_id("main-tray")
        .icon(icon)
        .tooltip("RustyBox VPN")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let app_handle = app.clone();
            match event.id().as_ref() {
                "show_window" => {
                    show_main_window(&app_handle);
                }
                "open_settings" => {
                    show_main_window(&app_handle);
                    let _ = app_handle.emit("navigate", "settings");
                }
                "toggle_connect" => {
                    tauri::async_runtime::spawn(async move {
                        if let Some(state) = app_handle.try_state::<AppState>() {
                            let current = state.vpn.get_state();
                            if current == crate::vpn::ConnectionState::Connected {
                                let _ = state.vpn.disconnect().await;
                            } else if current == crate::vpn::ConnectionState::Disconnected
                                || current == crate::vpn::ConnectionState::Error
                            {
                                let _ = state.vpn.connect().await;
                            }
                        }
                    });
                }
                "quit" => {
                    let app_clone = app_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Some(state) = app_clone.try_state::<AppState>() {
                            let _ = state.vpn.disconnect().await;
                        }
                        app_clone.exit(0);
                    });
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                toggle_main_window(app);
            }
        })
        .build(app)?;

    Ok(())
}

pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn toggle_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
    }
}
