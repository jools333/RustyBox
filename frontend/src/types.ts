export const APP_VERSION = "0.1.17";

export type ConnectionState = "disconnected" | "starting" | "connected" | "stopping" | "error";

export interface VlessConfig {
  uuid: string;
  flow?: string;
  network?: string;
  tls: boolean;
  sni?: string;
  fingerprint?: string;
  reality: boolean;
  public_key?: string;
  short_id?: string;
}

export interface ShadowsocksConfig {
  method: string;
  password: string;
}

export interface OpenvpnConfig {
  proto: string;
  username?: string;
  password?: string;
  ca_cert?: string;
  client_cert?: string;
  client_key?: string;
  cipher?: string;
  comp_lzo: boolean;
  tls_auth?: string;
  tls_crypt?: string;
  auth_user_pass_required: boolean;
  raw_ovpn?: string;
}

export type ProtocolConfig =
  | ({ type: "Vless" } & VlessConfig)
  | ({ type: "Shadowsocks" } & ShadowsocksConfig)
  | ({ type: "Openvpn" } & OpenvpnConfig);

export interface Profile {
  id: string;
  name: string;
  server: string;
  port: number;
  protocol: ProtocolConfig;
  latency_ms?: number;
  only_network_resources?: boolean;
}

export type RuleAction = "DIRECT" | "VPN";
export type RoutingMode = "domain_rules" | "global_vpn" | "global_direct";

export interface RoutingRule {
  id: string;
  pattern: string;
  action: RuleAction;
  enabled: boolean;
}

export interface ConnectionStatus {
  state: ConnectionState;
  active_profile: Profile | null;
  work_profile?: Profile | null;
  connected_since: string | null;
  error_message: string | null;
}

export interface HostsEntries {
  domains: string[];
  ip_cidrs: string[];
}

export interface CoreStatus {
  is_running: boolean;
  pid: number | null;
  version: string | null;
  binary_path: string | null;
}

export interface TunPrerequisites {
  is_supported: boolean;
  has_driver: boolean;
  is_elevated: boolean;
  details: string[];
}

export interface LogEntry {
  timestamp: string;
  level: string;
  message: string;
}

export interface AppConfig {
  version: number;
  active_profile_id: string | null;
  work_profile_id?: string | null;
  work_profile_enabled?: boolean;
  routing_mode: RoutingMode;
  default_route_action: RuleAction;
  rules: RoutingRule[];
  profiles: Profile[];
  remote_dns: string;
  direct_dns: string;
  close_to_tray: boolean;
  singbox_path: string | null;
  tun_interface_name: string;
  log_level: string;
  auto_check_updates?: boolean;
  auto_start?: boolean;
  auto_connect?: boolean;
}
