import React, { useState } from "react";
import { Plus, Trash2, Download, Upload } from "lucide-react";
import { RoutingRule, RuleAction, RoutingMode } from "../types";

interface RoutingViewProps {
  rules: RoutingRule[];
  routingMode: RoutingMode;
  defaultAction: RuleAction;
  onUpdateMode: (mode: RoutingMode) => void;
  onUpdateDefaultAction: (action: RuleAction) => void;
  onAddRule: (pattern: string, action: RuleAction) => Promise<void>;
  onUpdateRule: (rule: RoutingRule) => Promise<void>;
  onDeleteRule: (id: string) => Promise<void>;
  onImportRules: (json: string) => Promise<void>;
}

export const RoutingView: React.FC<RoutingViewProps> = ({
  rules,
  routingMode,
  defaultAction,
  onUpdateMode,
  onUpdateDefaultAction,
  onAddRule,
  onUpdateRule,
  onDeleteRule,
  onImportRules,
}) => {
  const [newPattern, setNewPattern] = useState("");
  const [newAction, setNewAction] = useState<RuleAction>("DIRECT");
  const [showImportModal, setShowImportModal] = useState(false);
  const [importJson, setImportJson] = useState("");

  const handleAdd = async () => {
    if (!newPattern.trim()) return;
    await onAddRule(newPattern.trim(), newAction);
    setNewPattern("");
  };

  const handleExport = () => {
    const json = JSON.stringify(rules, null, 2);
    const blob = new Blob([json], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "rustybox-routing-rules.json";
    a.click();
    URL.revokeObjectURL(url);
  };

  const handleImportSubmit = async () => {
    try {
      await onImportRules(importJson);
      setShowImportModal(false);
      setImportJson("");
    } catch (e: any) {
      alert("Invalid JSON format: " + e?.message);
    }
  };

  return (
    <div className="p-4 space-y-4">
      {/* Routing Mode Selector */}
      <div className="p-3 bg-slate-900 border border-slate-800 rounded-xl space-y-2">
        <label className="block text-xs font-semibold text-slate-400 uppercase tracking-wider">
          Routing Mode
        </label>
        <div className="grid grid-cols-3 gap-1 bg-slate-950 p-1 rounded-lg border border-slate-800">
          <button
            onClick={() => onUpdateMode("domain_rules")}
            className={`py-1.5 px-2 text-xs font-medium rounded-md transition-all ${
              routingMode === "domain_rules"
                ? "bg-orange-600 text-white shadow"
                : "text-slate-400 hover:text-slate-200"
            }`}
          >
            Domain Rules
          </button>
          <button
            onClick={() => onUpdateMode("global_vpn")}
            className={`py-1.5 px-2 text-xs font-medium rounded-md transition-all ${
              routingMode === "global_vpn"
                ? "bg-orange-600 text-white shadow"
                : "text-slate-400 hover:text-slate-200"
            }`}
          >
            Global VPN
          </button>
          <button
            onClick={() => onUpdateMode("global_direct")}
            className={`py-1.5 px-2 text-xs font-medium rounded-md transition-all ${
              routingMode === "global_direct"
                ? "bg-orange-600 text-white shadow"
                : "text-slate-400 hover:text-slate-200"
            }`}
          >
            Global Direct
          </button>
        </div>

        {routingMode === "domain_rules" && (
          <div className="flex items-center justify-between pt-1 text-xs text-slate-400">
            <span>Fallback Default Action:</span>
            <select
              value={defaultAction}
              onChange={(e) => onUpdateDefaultAction(e.target.value as RuleAction)}
              className="bg-slate-950 border border-slate-800 rounded px-2 py-1 text-xs text-slate-200 font-semibold"
            >
              <option value="VPN">VPN (Proxy)</option>
              <option value="DIRECT">DIRECT (Bypass)</option>
            </select>
          </div>
        )}
      </div>

      {/* Add New Rule Bar */}
      {routingMode === "domain_rules" && (
        <div className="p-3 bg-slate-900 border border-slate-800 rounded-xl space-y-2">
          <label className="block text-xs font-semibold text-slate-400 uppercase tracking-wider">
            Add Domain Rule
          </label>
          <div className="flex gap-2">
            <input
              type="text"
              value={newPattern}
              onChange={(e) => setNewPattern(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleAdd()}
              placeholder="e.g. example.com, *.example.org, google"
              className="flex-1 bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-xs text-slate-200 focus:outline-none focus:border-orange-500 font-mono"
            />
            <select
              value={newAction}
              onChange={(e) => setNewAction(e.target.value as RuleAction)}
              className="bg-slate-950 border border-slate-800 rounded-xl px-2 py-2 text-xs font-semibold text-slate-200"
            >
              <option value="DIRECT">DIRECT</option>
              <option value="VPN">VPN</option>
            </select>
            <button
              onClick={handleAdd}
              disabled={!newPattern.trim()}
              className="px-3 py-2 bg-orange-600 hover:bg-orange-500 disabled:opacity-50 text-white rounded-xl text-xs font-semibold shadow transition-all"
            >
              <Plus className="w-4 h-4" />
            </button>
          </div>
          <p className="text-[10px] text-slate-500">
            Rules apply from top to bottom. Domains matching DIRECT bypass VPN.
          </p>
        </div>
      )}

      {/* Rules Table / List */}
      <div className="space-y-2">
        <div className="flex items-center justify-between text-xs font-semibold text-slate-400 px-1">
          <span>Active Rules ({rules.length})</span>
          <div className="flex gap-2">
            <button
              onClick={handleExport}
              className="flex items-center gap-1 text-[11px] text-slate-400 hover:text-slate-200"
            >
              <Download className="w-3 h-3" />
              Export
            </button>
            <button
              onClick={() => setShowImportModal(true)}
              className="flex items-center gap-1 text-[11px] text-orange-400 hover:text-orange-300"
            >
              <Upload className="w-3 h-3" />
              Import
            </button>
          </div>
        </div>

        {rules.length === 0 ? (
          <div className="p-6 text-center text-xs text-slate-500 border border-dashed border-slate-800 rounded-xl">
            No domain rules added.
          </div>
        ) : (
          <div className="space-y-1.5 max-h-[340px] overflow-y-auto pr-0.5">
            {rules.map((rule) => (
              <div
                key={rule.id}
                className="p-2.5 bg-slate-900/60 border border-slate-800 rounded-xl flex items-center justify-between gap-2 text-xs"
              >
                <div className="flex items-center gap-2 min-w-0">
                  <input
                    type="checkbox"
                    checked={rule.enabled}
                    onChange={(e) =>
                      onUpdateRule({ ...rule, enabled: e.target.checked })
                    }
                    className="rounded border-slate-700 bg-slate-950 text-orange-600 focus:ring-0 cursor-pointer"
                  />
                  <span
                    className={`font-mono text-xs truncate ${
                      rule.enabled ? "text-slate-200" : "text-slate-600 line-through"
                    }`}
                  >
                    {rule.pattern}
                  </span>
                </div>

                <div className="flex items-center gap-2 shrink-0">
                  <button
                    onClick={() =>
                      onUpdateRule({
                        ...rule,
                        action: rule.action === "DIRECT" ? "VPN" : "DIRECT",
                      })
                    }
                    className={`px-2 py-0.5 rounded text-[10px] font-bold tracking-wider uppercase border ${
                      rule.action === "DIRECT"
                        ? "bg-emerald-950 text-emerald-400 border-emerald-800/60"
                        : "bg-indigo-950 text-indigo-400 border-indigo-800/60"
                    }`}
                  >
                    {rule.action}
                  </button>

                  <button
                    onClick={() => onDeleteRule(rule.id)}
                    className="p-1 text-slate-500 hover:text-rose-400 transition-colors"
                  >
                    <Trash2 className="w-3.5 h-3.5" />
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Import Modal */}
      {showImportModal && (
        <div className="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4">
          <div className="bg-slate-900 border border-slate-800 rounded-2xl w-full max-w-md p-5 space-y-3 shadow-2xl">
            <h3 className="text-sm font-bold text-white flex items-center gap-2">
              <Upload className="w-4 h-4 text-orange-400" />
              Import Routing Rules JSON
            </h3>
            <textarea
              rows={6}
              value={importJson}
              onChange={(e) => setImportJson(e.target.value)}
              placeholder='[ { "pattern": "example.com", "action": "DIRECT", "enabled": true } ]'
              className="w-full bg-slate-950 border border-slate-800 rounded-xl p-3 text-xs text-slate-200 font-mono"
            />
            <div className="flex justify-end gap-2">
              <button
                onClick={() => setShowImportModal(false)}
                className="px-3 py-1.5 text-xs text-slate-400 hover:text-slate-200"
              >
                Cancel
              </button>
              <button
                onClick={handleImportSubmit}
                disabled={!importJson.trim()}
                className="px-4 py-1.5 bg-orange-600 hover:bg-orange-500 text-white text-xs font-semibold rounded-xl"
              >
                Import Rules
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
