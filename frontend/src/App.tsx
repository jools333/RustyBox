import React, { useEffect, useState } from "react";
import { Header } from "./components/Header";
import { ConnectionPanel } from "./components/ConnectionPanel";
import { ProfilesView } from "./components/ProfilesView";
import { RoutingView } from "./components/RoutingView";
import { SettingsView } from "./components/SettingsView";
import { LogsView } from "./components/LogsView";
import { api, Update } from "./api";
import {
  AppConfig,
  ConnectionStatus,
  CoreStatus,
  LogEntry,
  Profile,
  RoutingRule,
  RuleAction,
  RoutingMode,
  TunPrerequisites,
} from "./types";

export const App: React.FC = () => {
  const [currentTab, setCurrentTab] = useState<string>("connect");
  const [status, setStatus] = useState<ConnectionStatus>({
    state: "disconnected",
    active_profile: null,
    connected_since: null,
    error_message: null,
  });
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [rules, setRules] = useState<RoutingRule[]>([]);
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [coreStatus, setCoreStatus] = useState<CoreStatus | null>(null);
  const [tunPrereqs, setTunPrereqs] = useState<TunPrerequisites | null>(null);
  const [availableUpdate, setAvailableUpdate] = useState<Update | null>(null);

  // Initial load
  useEffect(() => {
    loadAllData();
    // Poll status periodically (every 1.5s)
    const interval = setInterval(() => {
      fetchStatus();
    }, 1500);
    return () => clearInterval(interval);
  }, []);

  // Check for updates on startup if enabled
  useEffect(() => {
    const checkUpdatesOnStartup = async () => {
      try {
        const c = await api.getConfig();
        if (c.auto_check_updates !== false) {
          const update = await api.checkForUpdates();
          if (update) {
            setAvailableUpdate(update);
          }
        }
      } catch (e) {
        console.warn("Background update check skipped/failed:", e);
      }
    };
    checkUpdatesOnStartup();
  }, []);

  const loadAllData = async () => {
    try {
      const [s, p, r, c, l, cs, tp] = await Promise.all([
        api.getStatus(),
        api.getProfiles(),
        api.getRoutingRules(),
        api.getConfig(),
        api.getLogs(),
        api.getCoreStatus(),
        api.checkTunPrerequisites(),
      ]);
      setStatus(s);
      setProfiles(p);
      setRules(r);
      setConfig(c);
      setLogs(l);
      setCoreStatus(cs);
      setTunPrereqs(tp);
    } catch (e) {
      console.error("Failed to load data", e);
    }
  };

  const fetchStatus = async () => {
    try {
      const [s, cs] = await Promise.all([api.getStatus(), api.getCoreStatus()]);
      setStatus(s);
      setCoreStatus(cs);
    } catch (e) {
      console.error("Failed to fetch status", e);
    }
  };

  const handleConnect = async () => {
    try {
      await api.connectVpn();
      fetchStatus();
    } catch (e: any) {
      const msg = e?.message || String(e);
      if (msg.includes("Access is denied") || msg.includes("Administrator privileges")) {
        alert(
          "Administrator privileges required!\n\nTUN mode requires administrator privileges to create and configure the virtual network adapter on Windows.\n\nPlease close RustyBox, right-click its icon, and select 'Run as administrator'."
        );
      } else {
        alert("Connect failed: " + msg);
      }
    }
  };

  const handleDisconnect = async () => {
    try {
      await api.disconnectVpn();
      fetchStatus();
    } catch (e: any) {
      alert("Disconnect failed: " + (e?.message || String(e)));
    }
  };

  const handleSelectProfile = async (id: string) => {
    try {
      await api.selectProfile(id);
      await loadAllData();
    } catch (e: any) {
      alert("Failed to select profile: " + (e?.message || String(e)));
    }
  };

  const handleImportUri = async (uri: string) => {
    await api.importProfileUri(uri);
    await loadAllData();
  };

  const handleAddProfile = async (profile: Profile) => {
    await api.addProfile(profile);
    await loadAllData();
  };

  const handleDeleteProfile = async (id: string) => {
    if (confirm("Delete this server profile?")) {
      await api.deleteProfile(id);
      await loadAllData();
    }
  };

  const handleAddRule = async (pattern: string, action: RuleAction) => {
    await api.addRoutingRule(pattern, action);
    const updated = await api.getRoutingRules();
    setRules(updated);
  };

  const handleUpdateRule = async (rule: RoutingRule) => {
    await api.updateRoutingRule(rule);
    const updated = await api.getRoutingRules();
    setRules(updated);
  };

  const handleDeleteRule = async (id: string) => {
    await api.deleteRoutingRule(id);
    const updated = await api.getRoutingRules();
    setRules(updated);
  };

  const handleUpdateRoutingMode = async (mode: RoutingMode) => {
    if (!config) return;
    const updated = { ...config, routing_mode: mode };
    await api.saveConfig(updated);
    setConfig(updated);
  };

  const handleUpdateDefaultAction = async (action: RuleAction) => {
    if (!config) return;
    const updated = { ...config, default_route_action: action };
    await api.saveConfig(updated);
    setConfig(updated);
  };

  const handleImportRules = async (json: string) => {
    const parsed = JSON.parse(json);
    if (!Array.isArray(parsed)) throw new Error("Expected JSON array of rules");
    await api.saveRoutingRules(parsed);
    const updated = await api.getRoutingRules();
    setRules(updated);
  };

  const handleSaveConfig = async (newConfig: AppConfig) => {
    await api.saveConfig(newConfig);
    setConfig(newConfig);
    const [p, r, cs, tp] = await Promise.all([
      api.getProfiles(),
      api.getRoutingRules(),
      api.getCoreStatus(),
      api.checkTunPrerequisites(),
    ]);
    setProfiles(p);
    setRules(r);
    setCoreStatus(cs);
    setTunPrereqs(tp);
  };

  const refreshLogs = async () => {
    const l = await api.getLogs();
    setLogs(l);
  };

  const clearLogs = async () => {
    await api.clearLogs();
    setLogs([]);
  };

  return (
    <div className="flex flex-col h-screen w-screen bg-slate-950 text-slate-100 select-none overflow-hidden font-sans">
      <Header
        currentTab={currentTab}
        setCurrentTab={setCurrentTab}
        vpnState={status.state}
        hasUpdate={!!availableUpdate}
      />

      <main className="flex-1 overflow-y-auto">
        {currentTab === "connect" && (
          <ConnectionPanel
            status={status}
            profiles={profiles}
            tunPrereqs={tunPrereqs}
            onConnect={handleConnect}
            onDisconnect={handleDisconnect}
            onSelectProfile={handleSelectProfile}
          />
        )}

        {currentTab === "profiles" && (
          <ProfilesView
            profiles={profiles}
            activeProfileId={config?.active_profile_id || status.active_profile?.id || null}
            onSelect={handleSelectProfile}
            onImportUri={handleImportUri}
            onAdd={handleAddProfile}
            onDelete={handleDeleteProfile}
          />
        )}

        {currentTab === "routing" && config && (
          <RoutingView
            rules={rules}
            routingMode={config.routing_mode}
            defaultAction={config.default_route_action}
            onUpdateMode={handleUpdateRoutingMode}
            onUpdateDefaultAction={handleUpdateDefaultAction}
            onAddRule={handleAddRule}
            onUpdateRule={handleUpdateRule}
            onDeleteRule={handleDeleteRule}
            onImportRules={handleImportRules}
          />
        )}

        {currentTab === "settings" && config && (
          <SettingsView
            config={config}
            tunPrereqs={tunPrereqs}
            coreStatus={coreStatus}
            onSaveConfig={handleSaveConfig}
            availableUpdate={availableUpdate}
            onUpdateFound={setAvailableUpdate}
          />
        )}

        {currentTab === "logs" && (
          <LogsView
            logs={logs}
            onRefresh={refreshLogs}
            onClear={clearLogs}
            onExportReport={() => api.getDiagnosticReport()}
          />
        )}
      </main>
    </div>
  );
};
