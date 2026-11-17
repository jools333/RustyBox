use serde::{Deserialize, Serialize};
use tauri::{
    plugin::{Builder, PluginApi, PluginHandle, TauriPlugin},
    AppHandle, Manager, Runtime,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartVpnRequest {
    pub config: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VpnStatusResponse {
    pub running: bool,
    pub error: Option<String>,
}

pub struct RustyBoxVpn<R: Runtime> {
    #[allow(dead_code)]
    handle: PluginHandle<R>,
}

impl<R: Runtime> RustyBoxVpn<R> {
    pub fn start_vpn(&self, config: String) -> Result<(), String> {
        #[cfg(target_os = "android")]
        {
            self.handle
                .run_mobile_plugin::<serde_json::Value>("startVpn", StartVpnRequest { config })
                .map(|_| ())
                .map_err(|e| e.to_string())
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = config;
            Err("Android VPN plugin is only supported on Android".to_string())
        }
    }

    pub fn stop_vpn(&self) -> Result<(), String> {
        #[cfg(target_os = "android")]
        {
            self.handle
                .run_mobile_plugin::<serde_json::Value>("stopVpn", ())
                .map(|_| ())
                .map_err(|e| e.to_string())
        }
        #[cfg(not(target_os = "android"))]
        {
            Err("Android VPN plugin is only supported on Android".to_string())
        }
    }

    pub fn get_vpn_status(&self) -> Result<VpnStatusResponse, String> {
        #[cfg(target_os = "android")]
        {
            self.handle
                .run_mobile_plugin::<VpnStatusResponse>("getVpnStatus", ())
                .map_err(|e| e.to_string())
        }
        #[cfg(not(target_os = "android"))]
        {
            Ok(VpnStatusResponse {
                running: false,
                error: None,
            })
        }
    }
}

pub trait RustyBoxVpnExt<R: Runtime> {
    fn rustybox_vpn(&self) -> &RustyBoxVpn<R>;
}

impl<R: Runtime, T: Manager<R>> RustyBoxVpnExt<R> for T {
    fn rustybox_vpn(&self) -> &RustyBoxVpn<R> {
        self.state::<RustyBoxVpn<R>>().inner()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("rustybox-vpn")
        .setup(|app, api| {
            let vpn = setup_vpn(app, api)?;
            app.manage(vpn);
            Ok(())
        })
        .build()
}

fn setup_vpn<R: Runtime>(
    _app: &AppHandle<R>,
    api: PluginApi<R, ()>,
) -> Result<RustyBoxVpn<R>, Box<dyn std::error::Error>> {
    #[cfg(target_os = "android")]
    let handle = api.register_android_plugin("com.rustybox.vpn", "VpnPlugin")?;
    #[cfg(not(target_os = "android"))]
    let handle = {
        let _ = api;
        return Err("rustybox-vpn plugin is only supported on Android".into());
    };
    #[cfg(target_os = "android")]
    Ok(RustyBoxVpn { handle })
}
