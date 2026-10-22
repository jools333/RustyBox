import React, { useEffect, useState } from "react";
import { Power, Globe, AlertTriangle, Clock, Activity, ShieldAlert } from "lucide-react";
import { ConnectionStatus, Profile, TunPrerequisites } from "../types";

interface ConnectionPanelProps {
  status: ConnectionStatus;
  profiles: Profile[];
  tunPrereqs?: TunPrerequisites | null;
  onConnect: () => void;
  onDisconnect: () => void;
  onSelectProfile: (id: string) => void;
}

export const ConnectionPanel: React.FC<ConnectionPanelProps> = ({
  status,
  profiles,
  tunPrereqs,
  onConnect,
  onDisconnect,
  onSelectProfile,
}) => {
  const [uptime, setUptime] = useState<string>("00:00:00");

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

  return (
    <div className="flex flex-col items-center justify-between p-6 h-full max-w-sm mx-auto">
      {/* Top Profile Selector */}
      <div className="w-full">
        <label className="block text-xs font-semibold text-slate-400 mb-1.5 uppercase tracking-wider">
          Active Profile
        </label>
        {profiles.length === 0 ? (
          <div className="p-3 bg-slate-900 border border-slate-800 rounded-xl text-center text-xs text-slate-500">
            No profiles available. Please add or import one in the Profiles tab.
          </div>
        ) : (
          <select
            value={status.active_profile?.id || ""}
            disabled={isBusy}
            onChange={(e) => onSelectProfile(e.target.value)}
            className="w-full bg-slate-900 border border-slate-800 rounded-xl px-3 py-2.5 text-sm text-slate-200 focus:outline-none focus:border-orange-500 transition-colors cursor-pointer"
          >
            {profiles.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name} ({p.protocol.type.toUpperCase()} - {p.server}:{p.port})
              </option>
            ))}
          </select>
        )}

        {/* Admin Elevation Warning Banner */}
        {tunPrereqs && !tunPrereqs.is_elevated && (
          <div className="mt-3 p-3 bg-amber-950/40 border border-amber-800/60 rounded-xl flex items-start gap-2.5 text-amber-300 text-xs">
            <ShieldAlert className="w-4 h-4 shrink-0 text-amber-400 mt-0.5" />
            <div className="flex-1">
              <span className="font-semibold block text-amber-200">
                Administrator Rights Required
              </span>
              TUN mode needs administrator privileges to configure network routes. Please right-click RustyBox and choose "Run as administrator".
            </div>
          </div>
        )}
      </div>

      {/* Center Connect Button */}
      <div className="my-8 flex flex-col items-center">
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
        <p className="mt-4 text-xs font-medium text-slate-400 flex items-center gap-1.5">
          <Activity className="w-3.5 h-3.5 text-orange-400" />
          Status:{" "}
          <span className="capitalize font-semibold text-slate-200">
            {status.state}
          </span>
        </p>
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
              <p className="font-medium text-slate-200 truncate">TUN Mode</p>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
