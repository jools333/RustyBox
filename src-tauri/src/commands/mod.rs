use crate::app::AppState;
use crate::config::AppConfig;
use crate::core::CoreStatus;
use crate::logging::LogEntry;
use crate::platform::get_os_info;
use crate::profiles::Profile;
use crate::routing::{RoutingRule, RuleAction};
use crate::tun::TunPrerequisites;
use crate::vpn::ConnectionStatus;
use tauri::State;

#[tauri::command]
pub fn get_status(state: State<'_, AppState>) -> ConnectionStatus {
    state.vpn.get_status()
}

#[tauri::command]
pub async fn connect_vpn(state: State<'_, AppState>) -> Result<(), String> {
    state.vpn.connect().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn disconnect_vpn(state: State<'_, AppState>) -> Result<(), String> {
    state.vpn.disconnect().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_profiles(state: State<'_, AppState>) -> Vec<Profile> {
    state.profiles.list()
}

#[tauri::command]
pub fn add_profile(state: State<'_, AppState>, profile: Profile) -> Result<Profile, String> {
    state.profiles.add(profile.clone());
    let _ = state.config.update(|c| {
        c.profiles = state.profiles.list();
        if c.active_profile_id.is_none() {
            c.active_profile_id = Some(profile.id.clone());
        }
    });
    Ok(profile)
}

#[tauri::command]
pub fn import_profile_uri(state: State<'_, AppState>, uri: String) -> Result<Profile, String> {
    let profile = state.profiles.import_uri(&uri).map_err(|e| e.to_string())?;
    let _ = state.config.update(|c| {
        c.profiles = state.profiles.list();
        if c.active_profile_id.is_none() {
            c.active_profile_id = Some(profile.id.clone());
        }
    });
    Ok(profile)
}

#[tauri::command]
pub fn update_profile(state: State<'_, AppState>, profile: Profile) -> Result<(), String> {
    state.profiles.update(profile).map_err(|e| e.to_string())?;
    let _ = state.config.update(|c| {
        c.profiles = state.profiles.list();
    });
    Ok(())
}

#[tauri::command]
pub fn delete_profile(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.profiles.delete(&id).map_err(|e| e.to_string())?;
    let _ = state.config.update(|c| {
        c.profiles = state.profiles.list();
        if c.active_profile_id.as_deref() == Some(&id) {
            c.active_profile_id = c.profiles.first().map(|p| p.id.clone());
        }
    });
    Ok(())
}

#[tauri::command]
pub async fn select_profile(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state
        .vpn
        .switch_profile(&id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_routing_rules(state: State<'_, AppState>) -> Vec<RoutingRule> {
    state.routing.get_rules()
}

#[tauri::command]
pub fn save_routing_rules(
    state: State<'_, AppState>,
    rules: Vec<RoutingRule>,
) -> Result<(), String> {
    state.routing.set_rules(rules.clone());
    state
        .config
        .update(|c| {
            c.rules = rules;
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_routing_rule(
    state: State<'_, AppState>,
    pattern: String,
    action: RuleAction,
) -> Result<RoutingRule, String> {
    let rule = state.routing.add_rule(&pattern, action);
    let _ = state.config.update(|c| {
        c.rules = state.routing.get_rules();
    });
    Ok(rule)
}

#[tauri::command]
pub fn update_routing_rule(state: State<'_, AppState>, rule: RoutingRule) -> Result<(), String> {
    if state.routing.update_rule(rule) {
        let _ = state.config.update(|c| {
            c.rules = state.routing.get_rules();
        });
        Ok(())
    } else {
        Err("Rule not found".to_string())
    }
}

#[tauri::command]
pub fn delete_routing_rule(state: State<'_, AppState>, id: String) -> Result<(), String> {
    if state.routing.delete_rule(&id) {
        let _ = state.config.update(|c| {
            c.rules = state.routing.get_rules();
        });
        Ok(())
    } else {
        Err("Rule not found".to_string())
    }
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> AppConfig {
    state.config.get_config()
}

#[tauri::command]
pub fn save_config(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    config: AppConfig,
) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let autolaunch = app.autolaunch();
    if config.auto_start {
        if let Err(e) = autolaunch.enable() {
            tracing::warn!("Failed to enable autostart: {}", e);
        }
    } else {
        if let Err(e) = autolaunch.disable() {
            tracing::warn!("Failed to disable autostart: {}", e);
        }
    }

    state.routing.set_rules(config.rules.clone());
    state
        .routing
        .set_default_action(config.default_route_action);
    state.profiles.set_profiles(config.profiles.clone());
    if let Some(ref path) = config.singbox_path {
        state
            .core
            .set_custom_binary_path(Some(std::path::PathBuf::from(path)));
    } else {
        state.core.set_custom_binary_path(None);
    }
    state.config.save_config(config).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_logs(state: State<'_, AppState>) -> Vec<LogEntry> {
    state.log.get_logs()
}

#[tauri::command]
pub fn clear_logs(state: State<'_, AppState>) {
    state.log.clear();
}

#[tauri::command]
pub fn get_diagnostic_report(state: State<'_, AppState>) -> String {
    let os_info = get_os_info();
    let core_ver = state
        .core
        .get_version()
        .unwrap_or_else(|_| "Unavailable".to_string());
    state.log.export_diagnostic_report(&os_info, &core_ver)
}

#[tauri::command]
pub fn get_core_status(state: State<'_, AppState>) -> CoreStatus {
    state.core.get_status()
}

#[tauri::command]
pub fn check_tun_prerequisites(state: State<'_, AppState>) -> TunPrerequisites {
    let bin = state.core.find_binary();
    state.tun.check_prerequisites(bin.as_deref())
}

#[tauri::command]
pub fn relaunch_as_admin() -> Result<(), String> {
    crate::platform::relaunch_elevated()
}
