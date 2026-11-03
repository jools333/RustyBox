import React, { useEffect, useState } from "react";
import { Power, Globe, AlertTriangle, Clock, Activity, ShieldAlert, Briefcase } from "lucide-react";
import { AppConfig, ConnectionStatus, HostsEntries, Profile, TunPrerequisites } from "../types";
import { api } from "../api";

interface ConnectionPanelProps {
  status: ConnectionStatus;
  profiles: Profile[];
  config?: AppConfig | null;
  tunPrereqs?: TunPrerequisites | null;
  onConnect: () => void;
  onDisconnect: () => void;
  onSelectProfile: (id: string) => void;
  onConfigChange?: () => Promise<void>;
}

export const ConnectionPanel: React.FC<ConnectionPanelProps> = ({
  status,
  profiles,
  config,
  tunPrereqs,
  onConnect,
  onDisconnect,
  onSelectProfile,
  onConfigChange,
}) => {
  const [uptime, setUptime] = useState<string>("00:00:00");
  const [detectedHosts, setDetectedHosts] = useState<HostsEntries | null>(null);

  useEffect(() => {
    api.getDetectedWorkResources().then((res) => {
      setDetectedHosts(res);
    }).catch(() => {});
  }, []);

  useEffect(() => {
    if (status.state === "connected" && status.connected_since) {
      const interval = setInterval(() => {
        const start = new Date(status.connected_since!).getTime();
        const now = Date.now();
        const diff = Math.max(0, Math.floor((now - start) / 1000));
        const hours = Math.floor(diff / 3600);
        const mins = Math.floor((diff % 3600) / 60);
        const secs = diff % 60;
        setUptime(
          `${hours.toString().padStart(2, "0")}:${mins
            .toString()
            .padStart(2, "0")}:${secs.toString().padStart(2, "0")}`
        );
      }, 1000);
      return () => clearInterval(interval);
    } else {
      setUptime("00:00:00");
    }
  }, [status.state, status.connected_since]);

  const isConnected = status.state === "connected";
  const isBusy = status.state === "starting" || status.state === "stopping";

  const workEnabled = config?.work_profile_enabled ?? false;
  const workProfileId = config?.work_profile_id ?? null;

  const activeProfile = status.active_profile;
  const workProfile = workEnabled ? profiles.find((p) => p.id === workProfileId) : null;
  const isProfileMissingCerts = (p?: Profile | null) => {
    if (!p || p.protocol.type !== "Openvpn") return false;
    const cfg = p.protocol as any;
    return !cfg.ca_cert && !cfg.tls_auth && !cfg.tls_crypt;
  };
  const activeMissingCerts = isProfileMissingCerts(activeProfile);
  const workMissingCerts = isProfileMissingCerts(workProfile);

  const handleToggleWork = async (enabled: boolean) => {
    // If enabling and no work profile set, pick the first profile that isn't the primary one
    let targetId = workProfileId;
    if (enabled && !targetId) {
      const primaryId = status.active_profile?.id || profiles[0]?.id;
      const candidate = profiles.find((p) => p.id !== primaryId) || profiles[0];
      targetId = candidate ? candidate.id : null;
    }
    await api.setWorkProfile(targetId, enabled);
    if (onConfigChange) await onConfigChange();
  };

  const handleSelectWorkProfile = async (id: string) => {
    await api.setWorkProfile(id || null, true);
    if (onConfigChange) await onConfigChange();
  };

  return (
    <div className="flex flex-col items-center justify-between p-6 h-full max-w-sm mx-auto overflow-y-auto">
      {/* Top Profiles Selector */}
      <div className="w-full space-y-3">
        <div>
          <label className="block text-xs font-semibold text-slate-400 mb-1.5 uppercase tracking-wider">
            Основной VPN (Интернет)
          </label>
          {profiles.length === 0 ? (
            <div className="p-3 bg-slate-900 border border-slate-800 rounded-xl text-center text-xs text-slate-500">
              No profiles available. Please add or import one in the Profiles tab.
            </div>
          ) : (
            <select
              value={status.active_profile?.id || ""}
              disabled={isBusy || isConnected}
              onChange={(e) => onSelectProfile(e.target.value)}
              className="w-full bg-slate-900 border border-slate-800 rounded-xl px-3 py-2.5 text-sm text-slate-200 focus:outline-none focus:border-orange-500 transition-colors cursor-pointer"
            >
              {profiles.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name} ({p.protocol.type.toUpperCase()} - {p.server}:{p.port})
                  {p.only_network_resources ? " [Только сеть]" : ""}
                </option>
              ))}
            </select>
          )}
        </div>

        {/* Dual-VPN Work Profile Selector */}
        {profiles.length > 0 && (
          <div className="p-3 bg-slate-900/90 border border-slate-800 rounded-xl">
            <div className="flex items-center justify-between">
              <label className="flex items-center gap-2 cursor-pointer select-none">
                <input
                  type="checkbox"
                  checked={workEnabled}
                  disabled={isBusy || isConnected}
                  onChange={(e) => handleToggleWork(e.target.checked)}
                  className="rounded bg-slate-950 border-slate-700 text-emerald-500 focus:ring-0 cursor-pointer"
                />
                <span className="text-xs font-semibold text-slate-200 flex items-center gap-1.5">
                  <Briefcase className="w-3.5 h-3.5 text-emerald-400" />
                  Рабочий VPN (второе подключение)
                </span>
              </label>
              {detectedHosts && (
                <span
                  className="text-[10px] text-slate-400 font-mono bg-slate-800/80 px-1.5 py-0.5 rounded"
                  title={`${detectedHosts.domains.length} доменов и ${detectedHosts.ip_cidrs.length} IP адресов найдено в hosts`}
                >
                  hosts: {detectedHosts.domains.length + detectedHosts.ip_cidrs.length}
                </span>
              )}
            </div>

            {workEnabled && (
              <div className="space-y-1.5 mt-2.5 pt-2 border-t border-slate-800">
                <select
                  value={workProfileId || ""}
                  disabled={isBusy || isConnected}
                  onChange={(e) => handleSelectWorkProfile(e.target.value)}
                  className="w-full bg-slate-950 border border-slate-800 rounded-xl px-2.5 py-2 text-xs text-slate-200 focus:outline-none focus:border-emerald-500 cursor-pointer"
                >
                  <option value="">-- Выберите профиль рабочего VPN --</option>
                  {profiles.map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.name} ({p.protocol.type.toUpperCase()} - {p.server}:{p.port})
                    </option>
                  ))}
                </select>
                <p className="text-[10px] text-slate-400 leading-tight">
                  Только для ресурсов этой сети: запросы к доменам и IP из <span className="text-emerald-400 font-mono">hosts</span> пойдут в рабочий VPN, весь остальной трафик — в основной.
                </p>
              </div>
            )}
          </div>
        )}

        {(activeMissingCerts || workMissingCerts) && (
          <div className="mt-3 p-3 bg-rose-950/60 border border-rose-800/80 rounded-xl flex items-start gap-2.5 text-rose-300 text-xs">
            <AlertTriangle className="w-4 h-4 shrink-0 text-rose-400 mt-0.5" />
            <div className="flex-1">
              <span className="font-semibold block text-rose-200">
                {activeMissingCerts
                  ? `Профиль '${activeProfile?.name}' не содержит сертификата CA`
                  : `Рабочий профиль '${workProfile?.name}' не содержит сертификата CA`}
              </span>
              Перейдите во вкладку Профили и импортируйте .ovpn заново через кнопку «Import .ovpn».
            </div>
          </div>
        )}

        {/* Admin Elevation Warning Banner */}
        {tunPrereqs && !tunPrereqs.is_elevated && (
          <div className="mt-3 p-3 bg-amber-950/40 border border-amber-800/60 rounded-xl flex items-start gap-2.5 text-amber-300 text-xs">
            <ShieldAlert className="w-4 h-4 shrink-0 text-amber-400 mt-0.5" />
            <div className="flex-1">
              <span className="font-semibold block text-amber-200">
                Administrator Rights Required
              </span>
              TUN mode needs administrator or CAP_NET_ADMIN privileges to configure network routes.
              <button
                onClick={() => api.relaunchAsAdmin()}
                className="mt-2 block px-2.5 py-1 bg-amber-600 hover:bg-amber-500 text-slate-950 font-semibold rounded-lg text-[11px] transition-colors"
              >
                Restart as Administrator
              </button>
            </div>
          </div>
        )}
      </div>

      {/* Center Connect Button */}
      <div className="my-6 flex flex-col items-center">
        <button
          onClick={isConnected ? onDisconnect : onConnect}
          disabled={isBusy || profiles.length === 0}
          className={`relative group w-36 h-36 rounded-full flex flex-col items-center justify-center transition-all duration-300 shadow-2xl focus:outline-none ${
            isConnected
              ? "bg-emerald-600 hover:bg-emerald-500 text-white shadow-emerald-900/50 hover:scale-105"
              : isBusy
              ? "bg-amber-600 text-white cursor-wait animate-pulse"
              : "bg-slate-800 hover:bg-slate-700 text-slate-200 border-4 border-slate-700/50 hover:border-orange-500/50 hover:scale-105"
          }`}
        >
          <Power className={`w-12 h-12 mb-1 ${isBusy ? "animate-spin" : ""}`} />
          <span className="text-xs font-bold uppercase tracking-wider">
            {isConnected ? "Disconnect" : isBusy ? status.state : "Connect"}
          </span>
        </button>

        {/* State label */}
        <p className="mt-3 text-xs font-medium text-slate-400 flex items-center gap-1.5">
          <Activity className="w-3.5 h-3.5 text-orange-400" />
          Status:{" "}
          <span className="capitalize font-semibold text-slate-200">
            {status.state}
          </span>
        </p>

        {isConnected && status.work_profile && (
          <div className="mt-2 text-center text-[11px] text-emerald-400 font-medium bg-emerald-950/40 px-3 py-1 rounded-full border border-emerald-800/50">
            Dual-VPN: {status.active_profile?.name} + {status.work_profile.name}
          </div>
        )}
      </div>

      {/* Bottom Status Cards */}
      <div className="w-full space-y-3">
        {status.state === "error" && status.error_message && (
          <div className="p-3 bg-rose-950/40 border border-rose-800/60 rounded-xl flex items-start gap-2.5 text-rose-300 text-xs">
            <AlertTriangle className="w-4 h-4 shrink-0 text-rose-400 mt-0.5" />
            <div className="flex-1 break-words">
              <span className="font-semibold block text-rose-200">Connection Error</span>
              {status.error_message}
            </div>
          </div>
        )}

        <div className="grid grid-cols-2 gap-2 text-xs">
          <div className="p-3 bg-slate-900/60 border border-slate-800 rounded-xl flex items-center gap-2">
            <Clock className="w-4 h-4 text-orange-400 shrink-0" />
            <div>
              <p className="text-[10px] text-slate-400 uppercase">Uptime</p>
              <p className="font-mono font-medium text-slate-200">{uptime}</p>
            </div>
          </div>

          <div className="p-3 bg-slate-900/60 border border-slate-800 rounded-xl flex items-center gap-2">
            <Globe className="w-4 h-4 text-sky-400 shrink-0" />
            <div className="truncate">
              <p className="text-[10px] text-slate-400 uppercase">Routing</p>
              <p className="font-medium text-slate-200 truncate">
                {workEnabled ? "Dual TUN Split" : "TUN Mode"}
              </p>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
