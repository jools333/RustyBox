import React from "react";
import { Shield, Radio, Route, Settings, FileText } from "lucide-react";
import { ConnectionState } from "../types";

interface HeaderProps {
  currentTab: string;
  setCurrentTab: (tab: string) => void;
  vpnState: ConnectionState;
}

export const Header: React.FC<HeaderProps> = ({
  currentTab,
  setCurrentTab,
  vpnState,
}) => {
  const getBadgeClass = () => {
    switch (vpnState) {
      case "connected":
        return "bg-emerald-500/20 text-emerald-400 border-emerald-500/30";
      case "starting":
      case "stopping":
        return "bg-amber-500/20 text-amber-400 border-amber-500/30 animate-pulse";
      case "error":
        return "bg-rose-500/20 text-rose-400 border-rose-500/30";
      default:
        return "bg-slate-700 text-slate-400 border-slate-600";
    }
  };

  return (
    <header className="border-b border-slate-800 bg-slate-900/80 backdrop-blur sticky top-0 z-20">
      <div className="flex items-center justify-between px-4 py-3">
        <div className="flex items-center gap-2">
          <div className="w-8 h-8 rounded-lg bg-orange-600 flex items-center justify-center text-white shadow-lg shadow-orange-600/30">
            <Shield className="w-5 h-5" />
          </div>
          <div>
            <h1 className="text-base font-bold text-white leading-tight">RustyBox</h1>
            <p className="text-[10px] text-slate-400">sing-box VPN Core</p>
          </div>
        </div>

        <div
          className={`px-2.5 py-0.5 rounded-full text-xs font-medium border flex items-center gap-1.5 capitalize ${getBadgeClass()}`}
        >
          <span
            className={`w-1.5 h-1.5 rounded-full ${
              vpnState === "connected"
                ? "bg-emerald-400"
                : vpnState === "error"
                ? "bg-rose-400"
                : vpnState === "starting" || vpnState === "stopping"
                ? "bg-amber-400"
                : "bg-slate-400"
            }`}
          />
          {vpnState}
        </div>
      </div>

      {/* Tabs */}
      <nav className="flex px-2 gap-1 border-t border-slate-800/60 bg-slate-950/40">
        <button
          onClick={() => setCurrentTab("connect")}
          className={`flex items-center gap-1.5 px-3 py-2 text-xs font-medium border-b-2 transition-colors ${
            currentTab === "connect"
              ? "border-orange-500 text-orange-400"
              : "border-transparent text-slate-400 hover:text-slate-200"
          }`}
        >
          <Radio className="w-3.5 h-3.5" />
          Status
        </button>
        <button
          onClick={() => setCurrentTab("profiles")}
          className={`flex items-center gap-1.5 px-3 py-2 text-xs font-medium border-b-2 transition-colors ${
            currentTab === "profiles"
              ? "border-orange-500 text-orange-400"
              : "border-transparent text-slate-400 hover:text-slate-200"
          }`}
        >
          <Shield className="w-3.5 h-3.5" />
          Profiles
        </button>
        <button
          onClick={() => setCurrentTab("routing")}
          className={`flex items-center gap-1.5 px-3 py-2 text-xs font-medium border-b-2 transition-colors ${
            currentTab === "routing"
              ? "border-orange-500 text-orange-400"
              : "border-transparent text-slate-400 hover:text-slate-200"
          }`}
        >
          <Route className="w-3.5 h-3.5" />
          Routing
        </button>
        <button
          onClick={() => setCurrentTab("logs")}
          className={`flex items-center gap-1.5 px-3 py-2 text-xs font-medium border-b-2 transition-colors ${
            currentTab === "logs"
              ? "border-orange-500 text-orange-400"
              : "border-transparent text-slate-400 hover:text-slate-200"
          }`}
        >
          <FileText className="w-3.5 h-3.5" />
          Logs
        </button>
        <button
          onClick={() => setCurrentTab("settings")}
          className={`flex items-center gap-1.5 px-3 py-2 text-xs font-medium border-b-2 transition-colors ${
            currentTab === "settings"
              ? "border-orange-500 text-orange-400"
              : "border-transparent text-slate-400 hover:text-slate-200"
          }`}
        >
          <Settings className="w-3.5 h-3.5" />
          Settings
        </button>
      </nav>
    </header>
  );
};
