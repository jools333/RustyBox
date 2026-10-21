import React, { useState } from "react";
import { Save, CheckCircle2, AlertCircle, Wrench } from "lucide-react";
import { AppConfig, TunPrerequisites, CoreStatus } from "../types";

interface SettingsViewProps {
  config: AppConfig;
  tunPrereqs: TunPrerequisites | null;
  coreStatus: CoreStatus | null;
  onSaveConfig: (newConfig: AppConfig) => Promise<void>;
}

export const SettingsView: React.FC<SettingsViewProps> = ({
  config,
  tunPrereqs,
  coreStatus,
  onSaveConfig,
}) => {
  const [closeToTray, setCloseToTray] = useState(config.close_to_tray);
  const [tunName, setTunName] = useState(config.tun_interface_name);
  const [remoteDns, setRemoteDns] = useState(config.remote_dns);
  const [directDns, setDirectDns] = useState(config.direct_dns);
  const [singboxPath, setSingboxPath] = useState(config.singbox_path || "");
  const [logLevel, setLogLevel] = useState(config.log_level);
  const [savedMessage, setSavedMessage] = useState(false);

  const handleSave = async () => {
    const updated: AppConfig = {
      ...config,
      close_to_tray: closeToTray,
      tun_interface_name: tunName,
      remote_dns: remoteDns,
      direct_dns: directDns,
      singbox_path: singboxPath.trim() ? singboxPath.trim() : null,
      log_level: logLevel,
    };
    await onSaveConfig(updated);
    setSavedMessage(true);
    setTimeout(() => setSavedMessage(false), 2500);
  };

  return (
    <div className="p-4 space-y-4">
      {/* TUN & Core Health Status Card */}
      <div className="p-3.5 bg-slate-900 border border-slate-800 rounded-xl space-y-2 text-xs">
        <h3 className="font-semibold text-slate-200 flex items-center gap-1.5 uppercase tracking-wider text-[11px]">
          <Wrench className="w-3.5 h-3.5 text-orange-400" />
          System & Network Status
        </h3>

        <div className="space-y-1.5 pt-1">
          <div className="flex items-center justify-between">
            <span className="text-slate-400">Core Version:</span>
            <span className="font-mono text-slate-200 font-medium">
              {coreStatus?.version || "Checking..."}
            </span>
          </div>

          <div className="flex items-center justify-between">
            <span className="text-slate-400">TUN Driver Ready:</span>
            <span
              className={`flex items-center gap-1 font-medium ${
                tunPrereqs?.has_driver ? "text-emerald-400" : "text-amber-400"
              }`}
            >
              {tunPrereqs?.has_driver ? (
                <>
                  <CheckCircle2 className="w-3.5 h-3.5" /> Ready
                </>
              ) : (
                <>
                  <AlertCircle className="w-3.5 h-3.5" /> Missing
                </>
              )}
            </span>
          </div>

          <div className="flex items-center justify-between">
            <span className="text-slate-400">Elevation (Admin / Root):</span>
            <span
              className={`flex items-center gap-1 font-medium ${
                tunPrereqs?.is_elevated ? "text-emerald-400" : "text-amber-400"
              }`}
            >
              {tunPrereqs?.is_elevated ? (
                <>
                  <CheckCircle2 className="w-3.5 h-3.5" /> Elevated
                </>
              ) : (
                <>
                  <AlertCircle className="w-3.5 h-3.5" /> User mode
                </>
              )}
            </span>
          </div>
        </div>

        {tunPrereqs && tunPrereqs.details.length > 0 && (
          <div className="mt-2 pt-2 border-t border-slate-800 text-[10px] text-slate-400 space-y-0.5">
            {tunPrereqs.details.map((d, i) => (
              <p key={i}>• {d}</p>
            ))}
          </div>
        )}
      </div>

      {/* Settings Form */}
      <div className="space-y-3 text-xs">
        {/* Close to tray */}
        <div className="p-3 bg-slate-900 border border-slate-800 rounded-xl flex items-center justify-between">
          <div>
            <label className="font-medium text-slate-200 block">
              Close to System Tray
            </label>
            <p className="text-[10px] text-slate-400 mt-0.5">
              Keep VPN connection running in tray when window is closed
            </p>
          </div>
          <input
            type="checkbox"
            checked={closeToTray}
            onChange={(e) => setCloseToTray(e.target.checked)}
            className="w-4 h-4 rounded border-slate-700 bg-slate-950 text-orange-600 focus:ring-0 cursor-pointer"
          />
        </div>

        {/* Interface name */}
        <div className="p-3 bg-slate-900 border border-slate-800 rounded-xl space-y-1">
          <label className="font-medium text-slate-300 block">
            TUN Virtual Interface Name
          </label>
          <input
            type="text"
            value={tunName}
            onChange={(e) => setTunName(e.target.value)}
            className="w-full bg-slate-950 border border-slate-800 rounded-lg px-2.5 py-1.5 text-xs text-slate-200 font-mono"
          />
        </div>

        {/* DNS settings */}
        <div className="p-3 bg-slate-900 border border-slate-800 rounded-xl space-y-2">
          <label className="font-medium text-slate-300 block">DNS Resolvers</label>
          <div className="grid grid-cols-2 gap-2">
            <div>
              <span className="text-[10px] text-slate-400 block mb-0.5">
                VPN DNS (Remote)
              </span>
              <input
                type="text"
                value={remoteDns}
                onChange={(e) => setRemoteDns(e.target.value)}
                placeholder="1.1.1.1"
                className="w-full bg-slate-950 border border-slate-800 rounded-lg px-2.5 py-1.5 text-xs text-slate-200 font-mono"
              />
            </div>
            <div>
              <span className="text-[10px] text-slate-400 block mb-0.5">
                Direct DNS (Local)
              </span>
              <input
                type="text"
                value={directDns}
                onChange={(e) => setDirectDns(e.target.value)}
                placeholder="local"
                className="w-full bg-slate-950 border border-slate-800 rounded-lg px-2.5 py-1.5 text-xs text-slate-200 font-mono"
              />
            </div>
          </div>
        </div>

        {/* Custom sing-box binary path */}
        <div className="p-3 bg-slate-900 border border-slate-800 rounded-xl space-y-1">
          <label className="font-medium text-slate-300 block">
            Custom sing-box Binary Path (Optional)
          </label>
          <input
            type="text"
            value={singboxPath}
            onChange={(e) => setSingboxPath(e.target.value)}
            placeholder="Leave blank to use default ./bin/sing-box or PATH"
            className="w-full bg-slate-950 border border-slate-800 rounded-lg px-2.5 py-1.5 text-xs text-slate-200 font-mono"
          />
        </div>

        {/* Log level */}
        <div className="p-3 bg-slate-900 border border-slate-800 rounded-xl flex items-center justify-between">
          <label className="font-medium text-slate-300">Logging Level</label>
          <select
            value={logLevel}
            onChange={(e) => setLogLevel(e.target.value)}
            className="bg-slate-950 border border-slate-800 rounded-lg px-2 py-1 text-xs text-slate-200 font-mono"
          >
            <option value="trace">trace</option>
            <option value="debug">debug</option>
            <option value="info">info</option>
            <option value="warn">warn</option>
            <option value="error">error</option>
          </select>
        </div>
      </div>

      {/* Save Button */}
      <div className="flex items-center justify-between pt-2">
        {savedMessage ? (
          <span className="text-xs text-emerald-400 flex items-center gap-1 font-medium">
            <CheckCircle2 className="w-3.5 h-3.5" /> Settings saved successfully
          </span>
        ) : (
          <span />
        )}

        <button
          onClick={handleSave}
          className="flex items-center gap-1.5 px-4 py-2 bg-orange-600 hover:bg-orange-500 text-white rounded-xl text-xs font-semibold shadow transition-all ml-auto"
        >
          <Save className="w-3.5 h-3.5" />
          Save Settings
        </button>
      </div>
    </div>
  );
};
