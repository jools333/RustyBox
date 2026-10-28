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

const mockProfiles: Profile[] = [
  {
    id: "profile-vless-1",
    name: "Frankfurt (Fast VLESS Reality)",
    server: "fra-node1.rustybox.net",
    port: 443,
    latency_ms: 32,
    protocol: {
      type: "Vless",
      uuid: "d9e84b72-3c1a-4f5e-9e20-7b5f3a1d9c44",
      flow: "xtls-rprx-vision",
      tls: true,
      reality: true,
      public_key: "fG7_w0x-m1Z_sample_pubkey_abcdef1234567890",
      short_id: "6ba7b810",
      sni: "dl.google.com",
      fingerprint: "chrome",
    },
  },
  {
    id: "profile-ss-2",
    name: "Amsterdam (Shadowsocks 2022)",
    server: "ams-node2.rustybox.net",
    port: 8388,
    latency_ms: 45,
    protocol: {
      type: "Shadowsocks",
      method: "2022-blake3-aes-128-gcm",
      password: "sample_password_key",
    },
  },
  {
    id: "profile-vless-3",
    name: "Helsinki (VLESS Reality)",
    server: "hel-node3.rustybox.net",
    port: 443,
    latency_ms: 28,
    protocol: {
      type: "Vless",
      uuid: "e8a12c45-1234-5678-abcd-ef0123456789",
      flow: "xtls-rprx-vision",
      tls: true,
      reality: true,
      public_key: "kL9_w1x-n2Y_sample_pubkey_1234567890abcdef",
      short_id: "8ca9c921",
      sni: "gateway.icloud.com",
      fingerprint: "chrome",
    },
  },
];

const mockRules: RoutingRule[] = [
  {
    id: "rule-1",
    pattern: "*.ru",
    action: "DIRECT",
    enabled: true,
  },
  {
    id: "rule-2",
    pattern: "*.yandex.ru",
    action: "DIRECT",
    enabled: true,
  },
  {
    id: "rule-3",
    pattern: "*.vk.com",
    action: "DIRECT",
    enabled: true,
  },
  {
    id: "rule-4",
    pattern: "youtube.com",
    action: "VPN",
    enabled: true,
  },
  {
    id: "rule-5",
    pattern: "instagram.com",
    action: "VPN",
    enabled: true,
  },
  {
    id: "rule-6",
    pattern: "chatgpt.com",
    action: "VPN",
    enabled: true,
  },
];

export const api = {
  getStatus: async (): Promise<ConnectionStatus> => {
    if (!isTauri()) {
      const isDisconnected =
        typeof window !== "undefined" &&
        new URLSearchParams(window.location.search).get("state") === "disconnected";
      if (isDisconnected) {
        return {
          state: "disconnected",
          active_profile: mockProfiles[0],
          connected_since: null,
          error_message: null,
        };
      }
      return {
        state: "connected",
        active_profile: mockProfiles[0],
        connected_since: new Date(Date.now() - 5420000).toISOString(),
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
    if (!isTauri()) return mockProfiles;
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
    if (!isTauri()) return mockRules;
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
        active_profile_id: mockProfiles[0].id,
        routing_mode: "domain_rules",
        default_route_action: "VPN",
        rules: mockRules,
        profiles: mockProfiles,
        remote_dns: "1.1.1.1",
        direct_dns: "local",
        close_to_tray: true,
        singbox_path: null,
        tun_interface_name: "rustybox-tun",
        log_level: "info",
        auto_check_updates: true,
        auto_start: true,
      };
    }
    return invoke<AppConfig>("get_config");
  },

  saveConfig: async (config: AppConfig): Promise<void> => {
    if (!isTauri()) return;
    return invoke("save_config", { config });
  },

  getLogs: async (): Promise<LogEntry[]> => {
    if (!isTauri()) {
      return [
        { timestamp: "16:20:01", level: "info", message: "sing-box v1.14.3 core supervisor initialized" },
        { timestamp: "16:20:01", level: "info", message: "created virtual TUN adapter: rustybox-tun [172.19.0.1/30]" },
        { timestamp: "16:20:02", level: "info", message: "configured system routing table and DNS hijacking (port 53)" },
        { timestamp: "16:20:02", level: "info", message: "outbound/vless[proxy]: TLS handshake succeeded via REALITY" },
        { timestamp: "16:20:03", level: "info", message: "RustyBox TUN connection established successfully" },
      ];
    }
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
      return { is_running: true, pid: 14220, version: "1.14.3", binary_path: "sing-box" };
    }
    return invoke<CoreStatus>("get_core_status");
  },

  checkTunPrerequisites: async (): Promise<TunPrerequisites> => {
    if (!isTauri()) {
      return { is_supported: true, has_driver: true, is_elevated: true, details: ["Virtual adapter ready", "Administrator rights active"] };
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

  isAutostartEnabled: async (): Promise<boolean> => {
    if (!isTauri()) return true;
    try {
      return await invoke<boolean>("is_autostart_enabled");
    } catch (e) {
      console.warn("isAutostartEnabled check failed:", e);
      return false;
    }
  },

  enableAutostart: async (): Promise<void> => {
    if (!isTauri()) return;
    return invoke("enable_autostart");
  },

  disableAutostart: async (): Promise<void> => {
    if (!isTauri()) return;
    return invoke("disable_autostart");
  },

  relaunchAsAdmin: async (): Promise<void> => {
    if (!isTauri()) return;
    return invoke("relaunch_as_admin");
  },
};

export type { Update, DownloadEvent };
