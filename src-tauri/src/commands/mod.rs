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
pub fn import_ovpn_file(
    state: State<'_, AppState>,
    file_path: String,
    username: Option<String>,
    password: Option<String>,
) -> Result<Profile, String> {
    let path = std::path::Path::new(&file_path);
    let mut profile = crate::profiles::parse_ovpn_file(path).map_err(|e| e.to_string())?;

    if let crate::profiles::ProtocolConfig::Openvpn(ref mut cfg) = profile.protocol {
        if username.is_some() {
            cfg.username = username;
        }
        if password.is_some() {
            cfg.password = password;
        }
    }

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
pub fn pick_ovpn_file() -> Result<Option<String>, String> {
    Ok(crate::platform::pick_ovpn_file_dialog().map(|p| p.display().to_string()))
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub fn import_ovpn_content(
    state: State<'_, AppState>,
    name: Option<String>,
    content: String,
    username: Option<String>,
    password: Option<String>,
    ca_cert: Option<String>,
    client_cert: Option<String>,
    client_key: Option<String>,
) -> Result<Profile, String> {
    let mut profile =
        crate::profiles::parse_ovpn(&content, None, name.as_deref()).map_err(|e| e.to_string())?;

    if let crate::profiles::ProtocolConfig::Openvpn(ref mut cfg) = profile.protocol {
        if username.is_some() {
            cfg.username = username;
        }
        if password.is_some() {
            cfg.password = password;
        }
        if ca_cert.is_some() {
            cfg.ca_cert = ca_cert;
        }
        if client_cert.is_some() {
            cfg.client_cert = client_cert;
        }
        if client_key.is_some() {
            cfg.client_key = client_key;
        }
    }

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
pub async fn update_profile(
    state: State<'_, AppState>,
    profile: Profile,
) -> Result<Profile, String> {
    let profile_id = profile.id.clone();
    state.profiles.update(profile).map_err(|e| e.to_string())?;
    let updated = state
        .profiles
        .get(&profile_id)
        .ok_or_else(|| "Profile not found after update".to_string())?;

    let _ = state.config.update(|c| {
        c.profiles = state.profiles.list();
    });

    let cfg = state.config.get_config();
    let is_active = cfg.active_profile_id.as_deref() == Some(&profile_id);
    let is_work = cfg.work_profile_enabled && cfg.work_profile_id.as_deref() == Some(&profile_id);
    if is_active || is_work {
        let _ = state.vpn.reconnect().await;
    }

    Ok(updated)
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
    if let Err(e) = crate::platform::autostart::set_enabled(&app, config.auto_start) {
        tracing::warn!("Failed to set autostart: {}", e);
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

#[tauri::command]
pub fn enable_autostart(app: tauri::AppHandle) -> Result<(), String> {
    crate::platform::autostart::set_enabled(&app, true)
}

#[tauri::command]
pub fn disable_autostart(app: tauri::AppHandle) -> Result<(), String> {
    crate::platform::autostart::set_enabled(&app, false)
}

#[tauri::command]
pub fn is_autostart_enabled(app: tauri::AppHandle) -> Result<bool, String> {
    crate::platform::autostart::is_enabled(&app)
}

#[tauri::command]
pub fn set_work_profile(
    state: State<'_, AppState>,
    profile_id: Option<String>,
    enabled: bool,
) -> Result<AppConfig, String> {
    let mut config = state.config.get_config();
    config.work_profile_id = profile_id;
    config.work_profile_enabled = enabled;
    state
        .config
        .save_config(config.clone())
        .map_err(|e| e.to_string())?;
    Ok(config)
}

#[tauri::command]
pub fn toggle_profile_network_only(
    state: State<'_, AppState>,
    id: String,
    only_network: bool,
) -> Result<Profile, String> {
    let updated = state
        .profiles
        .set_only_network_resources(&id, only_network)
        .map_err(|e| e.to_string())?;

    let mut config = state.config.get_config();
    config.profiles = state.profiles.list();
    let _ = state.config.save_config(config);

    Ok(updated)
}

#[tauri::command]
pub fn get_detected_work_resources() -> crate::platform::HostsEntries {
    crate::platform::get_system_hosts_entries()
}
