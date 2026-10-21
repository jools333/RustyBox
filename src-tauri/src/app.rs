use crate::config::ConfigManager;
use crate::core::CoreManager;
use crate::logging::LogManager;
use crate::profiles::ProfileManager;
use crate::routing::RoutingManager;
use crate::tun::TunManager;
use crate::vpn::ConnectionManager;
use std::path::PathBuf;

#[derive(Clone)]
pub struct AppState {
    pub config: ConfigManager,
    pub profiles: ProfileManager,
    pub routing: RoutingManager,
    pub core: CoreManager,
    pub tun: TunManager,
    pub vpn: ConnectionManager,
    pub log: LogManager,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        let config_path = ConfigManager::default_path();
        let config = ConfigManager::new(config_path);
        let current_cfg = config.get_config();

        let log = LogManager::default();
        let profiles = ProfileManager::new(current_cfg.profiles);
        let routing = RoutingManager::new(current_cfg.rules, current_cfg.default_route_action);
        let core = CoreManager::new(log.clone());
        if let Some(ref path_str) = current_cfg.singbox_path {
            core.set_custom_binary_path(Some(PathBuf::from(path_str)));
        }

        let tun = TunManager::new();
        let vpn = ConnectionManager::new(
            profiles.clone(),
            routing.clone(),
            config.clone(),
            core.clone(),
            log.clone(),
        );

        Self {
            config,
            profiles,
            routing,
            core,
            tun,
            vpn,
            log,
        }
    }
}
