import { invoke } from "@tauri-apps/api/core";
import { check, Update, DownloadEvent } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import {
  AppConfig,
  ConnectionStatus,
  CoreStatus,
  LogEntry,
  Profile,
  RoutingRule,
  RuleAction,
  TunPrerequisites,
} from "./types";

const isTauri = (): boolean =>
  typeof window !== "undefined" &&
  ("__TAURI_INTERNALS__" in window || "__TAURI__" in window);

export const api = {
  getStatus: async (): Promise<ConnectionStatus> => {
    if (!isTauri()) {
      return {
        state: "disconnected",
        active_profile: null,
        connected_since: null,
        error_message: null,
      };
    }
    return invoke<ConnectionStatus>("get_status");
  },

  connectVpn: async (): Promise<void> => {
    if (!isTauri()) return;
    return invoke("connect_vpn");
  },

  disconnectVpn: async (): Promise<void> => {
    if (!isTauri()) return;
    return invoke("disconnect_vpn");
  },

  getProfiles: async (): Promise<Profile[]> => {
    if (!isTauri()) return [];
    return invoke<Profile[]>("get_profiles");
  },

  addProfile: async (profile: Profile): Promise<Profile> => {
    if (!isTauri()) return profile;
    return invoke<Profile>("add_profile", { profile });
  },

  importProfileUri: async (uri: string): Promise<Profile> => {
    if (!isTauri()) throw new Error("Tauri IPC only");
    return invoke<Profile>("import_profile_uri", { uri });
  },

  updateProfile: async (profile: Profile): Promise<void> => {
    if (!isTauri()) return;
    return invoke("update_profile", { profile });
  },

  deleteProfile: async (id: string): Promise<void> => {
    if (!isTauri()) return;
    return invoke("delete_profile", { id });
  },

  selectProfile: async (id: string): Promise<void> => {
    if (!isTauri()) return;
    return invoke("select_profile", { id });
  },

  getRoutingRules: async (): Promise<RoutingRule[]> => {
    if (!isTauri()) return [];
    return invoke<RoutingRule[]>("get_routing_rules");
  },

  saveRoutingRules: async (rules: RoutingRule[]): Promise<void> => {
    if (!isTauri()) return;
    return invoke("save_routing_rules", { rules });
  },

  addRoutingRule: async (pattern: string, action: RuleAction): Promise<RoutingRule> => {
    if (!isTauri()) throw new Error("Tauri IPC only");
    return invoke<RoutingRule>("add_routing_rule", { pattern, action });
  },

  updateRoutingRule: async (rule: RoutingRule): Promise<void> => {
    if (!isTauri()) return;
    return invoke("update_routing_rule", { rule });
  },

  deleteRoutingRule: async (id: string): Promise<void> => {
    if (!isTauri()) return;
    return invoke("delete_routing_rule", { id });
  },

  getConfig: async (): Promise<AppConfig> => {
    if (!isTauri()) {
      return {
        version: 1,
        active_profile_id: null,
        routing_mode: "domain_rules",
        default_route_action: "VPN",
        rules: [],
        profiles: [],
        remote_dns: "1.1.1.1",
        direct_dns: "local",
        close_to_tray: true,
        singbox_path: null,
        tun_interface_name: "rustybox-tun",
        log_level: "info",
        auto_check_updates: true,
      };
    }
    return invoke<AppConfig>("get_config");
  },

  saveConfig: async (config: AppConfig): Promise<void> => {
    if (!isTauri()) return;
    return invoke("save_config", { config });
  },

  getLogs: async (): Promise<LogEntry[]> => {
    if (!isTauri()) return [];
    return invoke<LogEntry[]>("get_logs");
  },

  clearLogs: async (): Promise<void> => {
    if (!isTauri()) return;
    return invoke("clear_logs");
  },

  getDiagnosticReport: async (): Promise<string> => {
    if (!isTauri()) return "Diagnostic report unavailable in browser preview";
    return invoke<string>("get_diagnostic_report");
  },

  getCoreStatus: async (): Promise<CoreStatus> => {
    if (!isTauri()) {
      return { is_running: false, pid: null, version: null, binary_path: null };
    }
    return invoke<CoreStatus>("get_core_status");
  },

  checkTunPrerequisites: async (): Promise<TunPrerequisites> => {
    if (!isTauri()) {
      return { is_supported: true, has_driver: false, is_elevated: false, details: [] };
    }
    return invoke<TunPrerequisites>("check_tun_prerequisites");
  },

  checkForUpdates: async (): Promise<Update | null> => {
    if (!isTauri()) return null;
    return check();
  },

  relaunchApp: async (): Promise<void> => {
    if (!isTauri()) return;
    return relaunch();
  },
};

export type { Update, DownloadEvent };
