import React, { useState } from "react";
import { Copy, Trash2, RefreshCw, Check, Terminal } from "lucide-react";
import { LogEntry } from "../types";

interface LogsViewProps {
  logs: LogEntry[];
  onRefresh: () => void;
  onClear: () => void;
  onExportReport: () => Promise<string>;
}

export const LogsView: React.FC<LogsViewProps> = ({
  logs,
  onRefresh,
  onClear,
  onExportReport,
}) => {
  const [copied, setCopied] = useState(false);

  const handleCopyReport = async () => {
    try {
      const report = await onExportReport();
      await navigator.clipboard.writeText(report);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Ignore
    }
  };

  const getLevelColor = (level: string) => {
    switch (level.toLowerCase()) {
      case "error":
        return "text-rose-400 bg-rose-950/40 border-rose-800/50";
      case "warn":
        return "text-amber-400 bg-amber-950/40 border-amber-800/50";
      case "debug":
        return "text-slate-400 bg-slate-800/40 border-slate-700/50";
      default:
        return "text-emerald-400 bg-emerald-950/40 border-emerald-800/50";
    }
  };

  return (
    <div className="p-4 space-y-3 flex flex-col h-[520px]">
      {/* Header action bar */}
      <div className="flex items-center justify-between text-xs">
        <div className="flex items-center gap-1.5 font-semibold text-slate-300">
          <Terminal className="w-3.5 h-3.5 text-orange-400" />
          <span>Core & VPN Logs ({logs.length})</span>
        </div>

        <div className="flex items-center gap-2">
          <button
            onClick={onRefresh}
            className="p-1.5 text-slate-400 hover:text-slate-200 transition-colors rounded-lg hover:bg-slate-800"
            title="Refresh logs"
          >
            <RefreshCw className="w-3.5 h-3.5" />
          </button>
          <button
            onClick={onClear}
            className="p-1.5 text-slate-400 hover:text-rose-400 transition-colors rounded-lg hover:bg-slate-800"
            title="Clear logs"
          >
            <Trash2 className="w-3.5 h-3.5" />
          </button>
          <button
            onClick={handleCopyReport}
            className="flex items-center gap-1 px-2.5 py-1 bg-slate-800 hover:bg-slate-700 text-slate-200 border border-slate-700 rounded-lg text-xs font-medium transition-all"
          >
            {copied ? (
              <>
                <Check className="w-3 h-3 text-emerald-400" />
                Copied
              </>
            ) : (
              <>
                <Copy className="w-3 h-3" />
                Copy Diagnostic Report
              </>
            )}
          </button>
        </div>
      </div>

      {/* Log Console Window */}
      <div className="flex-1 bg-slate-950 border border-slate-800 rounded-xl p-3 font-mono text-[11px] overflow-y-auto space-y-1 select-text">
        {logs.length === 0 ? (
          <div className="h-full flex items-center justify-center text-slate-600">
            No logs recorded yet.
          </div>
        ) : (
          logs.map((entry, idx) => (
            <div key={idx} className="flex items-start gap-2 leading-relaxed">
              <span className="text-slate-500 shrink-0 text-[10px]">
                {new Date(entry.timestamp).toLocaleTimeString()}
              </span>
              <span
                className={`px-1 py-0.2 rounded text-[9px] uppercase font-bold border shrink-0 ${getLevelColor(
                  entry.level
                )}`}
              >
                {entry.level}
              </span>
              <span className="text-slate-300 break-all">{entry.message}</span>
            </div>
          ))
        )}
      </div>

      <p className="text-[10px] text-slate-500">
        Passwords, UUIDs, and secret keys are automatically masked before saving or exporting.
      </p>
    </div>
  );
};
