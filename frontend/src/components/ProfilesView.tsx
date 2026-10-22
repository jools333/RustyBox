import React, { useState } from "react";
import { Plus, Trash2, Clipboard, Check, Server } from "lucide-react";
import { Profile, VlessConfig, ShadowsocksConfig } from "../types";

interface ProfilesViewProps {
  profiles: Profile[];
  activeProfileId: string | null;
  onSelect: (id: string) => void;
  onImportUri: (uri: string) => Promise<void>;
  onAdd: (profile: Profile) => Promise<void>;
  onDelete: (id: string) => Promise<void>;
}

export const ProfilesView: React.FC<ProfilesViewProps> = ({
  profiles,
  activeProfileId,
  onSelect,
  onImportUri,
  onAdd,
  onDelete,
}) => {
  const [showImportModal, setShowImportModal] = useState(false);
  const [uriInput, setUriInput] = useState("");
  const [importError, setImportError] = useState<string | null>(null);

  // Manual Add state
  const [showManualModal, setShowManualModal] = useState(false);
  const [name, setName] = useState("");
  const [server, setServer] = useState("");
  const [port, setPort] = useState(443);
  const [protoType, setProtoType] = useState<"Vless" | "Shadowsocks">("Vless");
  const [uuid, setUuid] = useState("");
  const [realityKey, setRealityKey] = useState("");
  const [sni, setSni] = useState("");
  const [ssMethod, setSsMethod] = useState("chacha20-ietf-poly1305");
  const [ssPassword, setSsPassword] = useState("");

  const handleImport = async () => {
    try {
      setImportError(null);
      await onImportUri(uriInput.trim());
      setUriInput("");
      setShowImportModal(false);
    } catch (e: any) {
      setImportError(e?.message || String(e));
    }
  };

  const handlePasteClipboard = async () => {
    try {
      const text = await navigator.clipboard.readText();
      if (text) {
        setUriInput(text);
      }
    } catch {
      // Ignore if clipboard permission denied
    }
  };

  const handleManualAdd = async () => {
    if (!name || !server || !port) return;

    let profile: Profile;
    if (protoType === "Vless") {
      const vlessCfg: VlessConfig = {
        uuid: uuid || "b831381d-6324-4d53-ad4f-8cda98b3f811",
        flow: "xtls-rprx-vision",
        network: "tcp",
        tls: true,
        sni: sni || server,
        fingerprint: "chrome",
        reality: !!realityKey,
        public_key: realityKey || undefined,
        short_id: undefined,
      };
      profile = {
        id: crypto.randomUUID(),
        name,
        server,
        port,
        protocol: { type: "Vless", ...vlessCfg },
      };
    } else {
      const ssCfg: ShadowsocksConfig = {
        method: ssMethod,
        password: ssPassword,
      };
      profile = {
        id: crypto.randomUUID(),
        name,
        server,
        port,
        protocol: { type: "Shadowsocks", ...ssCfg },
      };
    }

    await onAdd(profile);
    setShowManualModal(false);
  };

  return (
    <div className="p-4 space-y-4">
      {/* Action Buttons */}
      <div className="flex gap-2">
        <button
          onClick={() => setShowImportModal(true)}
          className="flex-1 flex items-center justify-center gap-1.5 px-3 py-2 bg-orange-600 hover:bg-orange-500 text-white rounded-xl text-xs font-semibold shadow-lg shadow-orange-600/20 transition-all"
        >
          <Clipboard className="w-3.5 h-3.5" />
          Import URI
        </button>
        <button
          onClick={() => setShowManualModal(true)}
          className="flex items-center justify-center gap-1.5 px-3 py-2 bg-slate-800 hover:bg-slate-700 text-slate-200 rounded-xl text-xs font-medium border border-slate-700 transition-all"
        >
          <Plus className="w-3.5 h-3.5" />
          Add Manual
        </button>
      </div>

      {/* Profiles List */}
      <div className="space-y-2">
        {profiles.length === 0 ? (
          <div className="p-8 text-center text-slate-500 border border-dashed border-slate-800 rounded-2xl">
            <Server className="w-8 h-8 mx-auto mb-2 opacity-50" />
            <p className="text-xs font-medium">No VPN profiles configured</p>
            <p className="text-[11px] text-slate-600 mt-1">
              Import a VLESS or Shadowsocks link to get started
            </p>
          </div>
        ) : (
          profiles.map((p) => {
            const isActive = p.id === activeProfileId;
            const isVless = p.protocol.type === "Vless";
            const isReality = isVless && (p.protocol as VlessConfig).reality;

            return (
              <div
                key={p.id}
                onClick={() => onSelect(p.id)}
                className={`p-3.5 rounded-xl border transition-all cursor-pointer flex items-center justify-between gap-3 ${
                  isActive
                    ? "bg-orange-950/20 border-orange-500/50 shadow-md shadow-orange-950/30"
                    : "bg-slate-900/60 border-slate-800 hover:border-slate-700"
                }`}
              >
                <div className="flex items-center gap-3 min-w-0">
                  <div
                    className={`w-4 h-4 rounded-full border flex items-center justify-center shrink-0 ${
                      isActive
                        ? "border-orange-500 bg-orange-500 text-white"
                        : "border-slate-600"
                    }`}
                  >
                    {isActive && <Check className="w-3 h-3 stroke-[3]" />}
                  </div>

                  <div className="min-w-0">
                    <div className="flex items-center gap-1.5">
                      <p className="text-xs font-semibold text-slate-200 truncate">
                        {p.name}
                      </p>
                      <span className="px-1.5 py-0.2 text-[9px] font-mono rounded bg-slate-800 text-slate-400 border border-slate-700">
                        {p.protocol.type.toUpperCase()}
                      </span>
                      {isReality && (
                        <span className="px-1.5 py-0.2 text-[9px] font-mono rounded bg-emerald-950 text-emerald-400 border border-emerald-800/50">
                          REALITY
                        </span>
                      )}
                    </div>
                    <p className="text-[11px] text-slate-400 truncate mt-0.5">
                      {p.server}:{p.port}
                    </p>
                  </div>
                </div>

                <div className="flex items-center gap-1 shrink-0">
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      onDelete(p.id);
                    }}
                    className="p-1.5 text-slate-500 hover:text-rose-400 transition-colors rounded-lg hover:bg-slate-800"
                    title="Delete profile"
                  >
                    <Trash2 className="w-3.5 h-3.5" />
                  </button>
                </div>
              </div>
            );
          })
        )}
      </div>

      {/* Import URI Modal */}
      {showImportModal && (
        <div className="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4">
          <div className="bg-slate-900 border border-slate-800 rounded-2xl w-full max-w-md p-5 space-y-4 shadow-2xl">
            <h3 className="text-sm font-bold text-white flex items-center gap-2">
              <Clipboard className="w-4 h-4 text-orange-400" />
              Import VLESS / Shadowsocks URI
            </h3>

            <div className="space-y-1.5">
              <div className="flex justify-between items-center text-xs text-slate-400">
                <span>Enter or paste URI link:</span>
                <button
                  onClick={handlePasteClipboard}
                  className="text-orange-400 hover:underline"
                >
                  Paste from clipboard
                </button>
              </div>
              <textarea
                rows={4}
                value={uriInput}
                onChange={(e) => setUriInput(e.target.value)}
                placeholder="vless://... or ss://..."
                className="w-full bg-slate-950 border border-slate-800 rounded-xl p-3 text-xs text-slate-200 font-mono focus:outline-none focus:border-orange-500"
              />
            </div>

            {importError && (
              <p className="text-xs text-rose-400 bg-rose-950/40 p-2.5 rounded-lg border border-rose-800/50">
                {importError}
              </p>
            )}

            <div className="flex justify-end gap-2 pt-2">
              <button
                onClick={() => setShowImportModal(false)}
                className="px-3 py-1.5 text-xs text-slate-400 hover:text-slate-200"
              >
                Cancel
              </button>
              <button
                onClick={handleImport}
                disabled={!uriInput.trim()}
                className="px-4 py-1.5 bg-orange-600 hover:bg-orange-500 disabled:opacity-50 text-white text-xs font-semibold rounded-xl"
              >
                Import
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Manual Add Modal */}
      {showManualModal && (
        <div className="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4">
          <div className="bg-slate-900 border border-slate-800 rounded-2xl w-full max-w-md p-5 space-y-3.5 shadow-2xl max-h-[90vh] overflow-y-auto">
            <h3 className="text-sm font-bold text-white flex items-center gap-2">
              <Server className="w-4 h-4 text-orange-400" />
              Add Server Profile
            </h3>

            <div className="space-y-2 text-xs">
              <div>
                <label className="block text-slate-400 mb-1">Profile Name</label>
                <input
                  type="text"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="My Server"
                  className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-slate-200"
                />
              </div>

              <div className="grid grid-cols-3 gap-2">
                <div className="col-span-2">
                  <label className="block text-slate-400 mb-1">Server Host / IP</label>
                  <input
                    type="text"
                    value={server}
                    onChange={(e) => setServer(e.target.value)}
                    placeholder="example.com"
                    className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-slate-200"
                  />
                </div>
                <div>
                  <label className="block text-slate-400 mb-1">Port</label>
                  <input
                    type="number"
                    value={port}
                    onChange={(e) => setPort(parseInt(e.target.value) || 443)}
                    className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-slate-200"
                  />
                </div>
              </div>

              <div>
                <label className="block text-slate-400 mb-1">Protocol</label>
                <select
                  value={protoType}
                  onChange={(e) => setProtoType(e.target.value as any)}
                  className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-slate-200"
                >
                  <option value="Vless">VLESS (with REALITY / TLS)</option>
                  <option value="Shadowsocks">Shadowsocks</option>
                </select>
              </div>

              {protoType === "Vless" ? (
                <>
                  <div>
                    <label className="block text-slate-400 mb-1">UUID</label>
                    <input
                      type="text"
                      value={uuid}
                      onChange={(e) => setUuid(e.target.value)}
                      placeholder="b831381d-6324-4d53-ad4f-8cda98b3f811"
                      className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-slate-200 font-mono text-[11px]"
                    />
                  </div>
                  <div>
                    <label className="block text-slate-400 mb-1">SNI / Server Name</label>
                    <input
                      type="text"
                      value={sni}
                      onChange={(e) => setSni(e.target.value)}
                      placeholder="www.microsoft.com"
                      className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-slate-200"
                    />
                  </div>
                  <div>
                    <label className="block text-slate-400 mb-1">REALITY Public Key (Optional)</label>
                    <input
                      type="text"
                      value={realityKey}
                      onChange={(e) => setRealityKey(e.target.value)}
                      placeholder="Public key for REALITY"
                      className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-slate-200 font-mono text-[11px]"
                    />
                  </div>
                </>
              ) : (
                <>
                  <div>
                    <label className="block text-slate-400 mb-1">Method / Cipher</label>
                    <select
                      value={ssMethod}
                      onChange={(e) => setSsMethod(e.target.value)}
                      className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-slate-200"
                    >
                      <option value="chacha20-ietf-poly1305">chacha20-ietf-poly1305</option>
                      <option value="aes-256-gcm">aes-256-gcm</option>
                      <option value="aes-128-gcm">aes-128-gcm</option>
                      <option value="2022-blake3-aes-128-gcm">2022-blake3-aes-128-gcm</option>
                      <option value="2022-blake3-aes-256-gcm">2022-blake3-aes-256-gcm</option>
                      <option value="2022-blake3-chacha20-poly1305">2022-blake3-chacha20-poly1305</option>
                    </select>
                  </div>
                  <div>
                    <label className="block text-slate-400 mb-1">Password</label>
                    <input
                      type="password"
                      value={ssPassword}
                      onChange={(e) => setSsPassword(e.target.value)}
                      className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-slate-200"
                    />
                  </div>
                </>
              )}
            </div>

            <div className="flex justify-end gap-2 pt-2">
              <button
                onClick={() => setShowManualModal(false)}
                className="px-3 py-1.5 text-xs text-slate-400 hover:text-slate-200"
              >
                Cancel
              </button>
              <button
                onClick={handleManualAdd}
                disabled={!name || !server}
                className="px-4 py-1.5 bg-orange-600 hover:bg-orange-500 disabled:opacity-50 text-white text-xs font-semibold rounded-xl"
              >
                Save Profile
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
