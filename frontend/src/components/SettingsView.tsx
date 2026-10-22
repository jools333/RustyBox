import React, { useState, useEffect } from "react";
import {
  Save,
  CheckCircle2,
  AlertCircle,
  Wrench,
  RefreshCw,
  Download,
  ArrowUpCircle,
} from "lucide-react";
import { AppConfig, TunPrerequisites, CoreStatus, APP_VERSION } from "../types";
import { api, Update } from "../api";

interface SettingsViewProps {
  config: AppConfig;
  tunPrereqs: TunPrerequisites | null;
  coreStatus: CoreStatus | null;
  onSaveConfig: (newConfig: AppConfig) => Promise<void>;
  availableUpdate?: Update | null;
  onUpdateFound?: (update: Update | null) => void;
}

export const SettingsView: React.FC<SettingsViewProps> = ({
  config,
  tunPrereqs,
  coreStatus,
  onSaveConfig,
  availableUpdate,
  onUpdateFound,
}) => {
  const [closeToTray, setCloseToTray] = useState(config.close_to_tray);
  const [autoCheckUpdates, setAutoCheckUpdates] = useState(
    config.auto_check_updates !== false
  );
  const [tunName, setTunName] = useState(config.tun_interface_name);
  const [remoteDns, setRemoteDns] = useState(config.remote_dns);
  const [directDns, setDirectDns] = useState(config.direct_dns);
  const [singboxPath, setSingboxPath] = useState(config.singbox_path || "");
  const [logLevel, setLogLevel] = useState(config.log_level);
  const [savedMessage, setSavedMessage] = useState(false);

  // Updater states
  const [updateStatus, setUpdateStatus] = useState<
    "idle" | "checking" | "up-to-date" | "available" | "downloading" | "ready" | "error"
  >(availableUpdate ? "available" : "idle");
  const [currentUpdate, setCurrentUpdate] = useState<Update | null>(
    availableUpdate || null
  );
  const [downloadProgress, setDownloadProgress] = useState<number>(0);
  const [updateError, setUpdateError] = useState<string | null>(null);

  useEffect(() => {
    if (availableUpdate) {
      setCurrentUpdate(availableUpdate);
      setUpdateStatus("available");
    }
  }, [availableUpdate]);

  const handleCheckUpdates = async () => {
    setUpdateStatus("checking");
    setUpdateError(null);
    try {
      const update = await api.checkForUpdates();
      if (update) {
        setCurrentUpdate(update);
        setUpdateStatus("available");
        onUpdateFound?.(update);
      } else {
        setCurrentUpdate(null);
        setUpdateStatus("up-to-date");
        onUpdateFound?.(null);
      }
    } catch (err: any) {
      setUpdateStatus("error");
      setUpdateError(err?.message || String(err));
    }
  };

  const handleInstallUpdate = async () => {
    if (!currentUpdate) return;
    setUpdateStatus("downloading");
    setDownloadProgress(0);
    setUpdateError(null);

    try {
      // 1. Ensure VPN is disconnected before replacing binaries
      await api.disconnectVpn();

      // 2. Download and install
      let totalLength = 0;
      let downloadedLength = 0;

      await currentUpdate.downloadAndInstall((event) => {
        if (event.event === "Started") {
          totalLength = event.data.contentLength || 0;
        } else if (event.event === "Progress") {
          downloadedLength += event.data.chunkLength;
          if (totalLength > 0) {
            setDownloadProgress(
              Math.min(100, Math.round((downloadedLength / totalLength) * 100))
            );
          }
        } else if (event.event === "Finished") {
          setDownloadProgress(100);
        }
      });

      setUpdateStatus("ready");
      // 3. Relaunch application
      await api.relaunchApp();
    } catch (err: any) {
      setUpdateStatus("error");
      setUpdateError(err?.message || String(err));
    }
  };

  const handleSave = async () => {
    const updated: AppConfig = {
      ...config,
      close_to_tray: closeToTray,
      auto_check_updates: autoCheckUpdates,
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

      {/* Software Updates Card */}
      <div className="p-3.5 bg-slate-900 border border-slate-800 rounded-xl space-y-2.5 text-xs">
        <div className="flex items-center justify-between">
          <h3 className="font-semibold text-slate-200 flex items-center gap-1.5 uppercase tracking-wider text-[11px]">
            <ArrowUpCircle className="w-3.5 h-3.5 text-orange-400" />
            Software Updates
          </h3>
          <span className="text-[10px] text-slate-400 font-mono">
            App Version: v{APP_VERSION}
          </span>
        </div>

        {/* Status display */}
        {updateStatus === "idle" && (
          <div className="flex items-center justify-between pt-1">
            <span className="text-slate-400">Check GitHub for new releases</span>
            <button
              onClick={handleCheckUpdates}
              className="flex items-center gap-1.5 px-3 py-1.5 bg-slate-800 hover:bg-slate-700 text-slate-200 rounded-lg font-medium transition-all"
            >
              <RefreshCw className="w-3 h-3" />
              Check for Updates
            </button>
          </div>
        )}

        {updateStatus === "checking" && (
          <div className="flex items-center gap-2 py-1.5 text-slate-300">
            <RefreshCw className="w-3.5 h-3.5 text-orange-400 animate-spin" />
            <span>Checking GitHub Releases for updates...</span>
          </div>
        )}

        {updateStatus === "up-to-date" && (
          <div className="flex items-center justify-between pt-1">
            <div className="flex items-center gap-1.5 text-emerald-400">
              <CheckCircle2 className="w-3.5 h-3.5" />
              <span>You have the latest version (v{APP_VERSION})</span>
            </div>
            <button
              onClick={handleCheckUpdates}
              className="text-[11px] text-slate-400 hover:text-slate-200 underline"
            >
              Check again
            </button>
          </div>
        )}

        {updateStatus === "error" && (
          <div className="space-y-1.5 pt-1">
            <div className="flex items-center gap-1.5 text-rose-400">
              <AlertCircle className="w-3.5 h-3.5 shrink-0" />
              <span className="truncate">{updateError || "Failed to check for updates"}</span>
            </div>
            <button
              onClick={handleCheckUpdates}
              className="flex items-center gap-1.5 px-3 py-1 bg-slate-800 hover:bg-slate-700 text-slate-200 rounded-lg text-[11px] font-medium"
            >
              <RefreshCw className="w-3 h-3" />
              Retry
            </button>
          </div>
        )}

        {updateStatus === "available" && currentUpdate && (
          <div className="space-y-2.5 pt-1">
            <div className="p-2.5 bg-orange-950/30 border border-orange-500/30 rounded-lg">
              <div className="flex items-center justify-between">
                <span className="font-semibold text-orange-400 flex items-center gap-1">
                  New Version Available: v{currentUpdate.version}
                </span>
                {currentUpdate.date && (
                  <span className="text-[10px] text-slate-400 font-mono">
                    {new Date(currentUpdate.date).toLocaleDateString()}
                  </span>
                )}
              </div>
              {currentUpdate.body && (
                <div className="mt-2 text-[11px] text-slate-300 bg-slate-950/60 p-2 rounded border border-slate-800/80 max-h-24 overflow-y-auto whitespace-pre-wrap font-sans">
                  {currentUpdate.body}
                </div>
              )}
            </div>

            <p className="text-[10px] text-slate-400">
              Installing the update will disconnect active VPN connections and restart RustyBox.
            </p>

            <button
              onClick={handleInstallUpdate}
              className="w-full flex items-center justify-center gap-2 py-2 bg-orange-600 hover:bg-orange-500 text-white rounded-xl font-semibold shadow transition-all"
            >
              <Download className="w-3.5 h-3.5" />
              Download & Install Update
            </button>
          </div>
        )}

        {updateStatus === "downloading" && (
          <div className="space-y-2 py-1">
            <div className="flex items-center justify-between text-slate-300">
              <span className="flex items-center gap-1.5">
                <Download className="w-3.5 h-3.5 text-orange-400 animate-bounce" />
                Downloading update package...
              </span>
              <span className="font-mono text-orange-400 font-bold">{downloadProgress}%</span>
            </div>
            <div className="w-full h-1.5 bg-slate-800 rounded-full overflow-hidden">
              <div
                className="h-full bg-orange-500 transition-all duration-300 rounded-full"
                style={{ width: `${downloadProgress}%` }}
              />
            </div>
          </div>
        )}

        {updateStatus === "ready" && (
          <div className="flex items-center gap-2 py-1.5 text-emerald-400">
            <CheckCircle2 className="w-3.5 h-3.5" />
            <span>Update downloaded! Applying installer and restarting RustyBox...</span>
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

        {/* Auto check updates toggle */}
        <div className="p-3 bg-slate-900 border border-slate-800 rounded-xl flex items-center justify-between">
          <div>
            <label className="font-medium text-slate-200 block">
              Auto-check for Updates
            </label>
            <p className="text-[10px] text-slate-400 mt-0.5">
              Automatically check GitHub for new releases on application startup
            </p>
          </div>
          <input
            type="checkbox"
            checked={autoCheckUpdates}
            onChange={(e) => setAutoCheckUpdates(e.target.checked)}
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
