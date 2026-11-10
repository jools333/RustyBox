import React, { useState, useRef } from "react";
import { Plus, Trash2, Clipboard, Check, Server, FileText, Lock, Pencil, X } from "lucide-react";
import { Profile, VlessConfig, ShadowsocksConfig, OpenvpnConfig } from "../types";
import { api } from "../api";

interface ProfilesViewProps {
  profiles: Profile[];
  activeProfileId: string | null;
  onSelect: (id: string) => void;
  onImportUri: (uri: string) => Promise<void>;
  onAdd: (profile: Profile) => Promise<void>;
  onUpdate?: (profile: Profile) => Promise<void>;
  onDelete: (id: string) => Promise<void>;
  onReload?: () => Promise<void>;
}

export const ProfilesView: React.FC<ProfilesViewProps> = ({
  profiles,
  activeProfileId,
  onSelect,
  onImportUri,
  onAdd,
  onUpdate,
  onDelete,
  onReload,
}) => {
  const [showImportModal, setShowImportModal] = useState(false);
  const [uriInput, setUriInput] = useState("");
  const [importError, setImportError] = useState<string | null>(null);

  // OpenVPN file input & credentials modal
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [showOvpnModal, setShowOvpnModal] = useState(false);
  const [ovpnFilePath, setOvpnFilePath] = useState("");
  const [ovpnContent, setOvpnContent] = useState("");
  const [ovpnUsername, setOvpnUsername] = useState("");
  const [ovpnPassword, setOvpnPassword] = useState("");
  const [ovpnError, setOvpnError] = useState<string | null>(null);

  // Manual Add state
  const [showManualModal, setShowManualModal] = useState(false);
  const [name, setName] = useState("");
  const [server, setServer] = useState("");
  const [port, setPort] = useState(443);
  const [protoType, setProtoType] = useState<"Vless" | "Shadowsocks">("Vless");
  const [uuid, setUuid] = useState("");
  const [vlessFlow, setVlessFlow] = useState("xtls-rprx-vision");
  const [realityKey, setRealityKey] = useState("");
  const [sni, setSni] = useState("");
  const [ssMethod, setSsMethod] = useState("chacha20-ietf-poly1305");
  const [ssPassword, setSsPassword] = useState("");
  const [customMtu, setCustomMtu] = useState<string>("");

  // Edit Profile state
  const [editingProfile, setEditingProfile] = useState<Profile | null>(null);
  const [editName, setEditName] = useState("");
  const [editServer, setEditServer] = useState("");
  const [editPort, setEditPort] = useState(443);
  const [editError, setEditError] = useState<string | null>(null);
  const [isSavingEdit, setIsSavingEdit] = useState(false);

  // VLESS edit fields
  const [editUuid, setEditUuid] = useState("");
  const [editFlow, setEditFlow] = useState("");
  const [editSni, setEditSni] = useState("");
  const [editFingerprint, setEditFingerprint] = useState("chrome");
  const [editNetwork, setEditNetwork] = useState("tcp");
  const [editTls, setEditTls] = useState(true);
  const [editReality, setEditReality] = useState(true);
  const [editPublicKey, setEditPublicKey] = useState("");
  const [editShortId, setEditShortId] = useState("");

  // Shadowsocks edit fields
  const [editSsMethod, setEditSsMethod] = useState("chacha20-ietf-poly1305");
  const [editSsPassword, setEditSsPassword] = useState("");

  // OpenVPN edit fields
  const [editOvpnProto, setEditOvpnProto] = useState("udp");
  const [editOvpnUsername, setEditOvpnUsername] = useState("");
  const [editOvpnPassword, setEditOvpnPassword] = useState("");

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

  const handleImportOvpnClick = async () => {
    if (api.isTauri()) {
      try {
        const filePath = await api.pickOvpnFile();
        if (filePath) {
          try {
            setOvpnError(null);
            await api.importOvpnFile(filePath);
            if (onReload) await onReload();
          } catch (err: any) {
            const msg = err?.message || String(err);
            setOvpnFilePath(filePath);
            setOvpnContent("");
            setOvpnError(msg);
            setShowOvpnModal(true);
          }
        }
        return;
      } catch (e) {
        console.warn("Native file picker failed, falling back to browser picker:", e);
      }
    }
    fileInputRef.current?.click();
  };

  const handleFileChange = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    // Reset input so re-selecting the same file fires onChange again
    e.target.value = "";

    const nativePath = (file as any).path as string | undefined;

    if (nativePath) {
      try {
        setOvpnError(null);
        await api.importOvpnFile(nativePath);
        if (onReload) await onReload();
      } catch (err: any) {
        const msg = err?.message || String(err);
        // If credentials are required or error occurred, open modal for username/password
        setOvpnFilePath(nativePath);
        setOvpnContent("");
        setOvpnError(msg);
        setShowOvpnModal(true);
      }
    } else {
      // Browser fallback (reading text)
      try {
        const text = await file.text();
        setOvpnFilePath("");
        setOvpnContent(text);
        setOvpnError(null);
        await api.importOvpnContent(text, file.name.replace(/\.ovpn$/i, ""));
        if (onReload) await onReload();
      } catch (err: any) {
        setOvpnError(err?.message || String(err));
        setShowOvpnModal(true);
      }
    }
  };

  const handleOvpnSubmit = async () => {
    try {
      setOvpnError(null);
      if (ovpnFilePath) {
        await api.importOvpnFile(
          ovpnFilePath,
          ovpnUsername.trim() || undefined,
          ovpnPassword || undefined
        );
      } else if (ovpnContent) {
        await api.importOvpnContent(
          ovpnContent,
          undefined,
          ovpnUsername.trim() || undefined,
          ovpnPassword || undefined
        );
      }
      setShowOvpnModal(false);
      setOvpnFilePath("");
      setOvpnContent("");
      setOvpnUsername("");
      setOvpnPassword("");
      if (onReload) await onReload();
    } catch (err: any) {
      setOvpnError(err?.message || String(err));
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

    const parsedMtu = parseInt(customMtu, 10);
    const mtuVal = !isNaN(parsedMtu) && parsedMtu > 0 ? parsedMtu : undefined;

    let profile: Profile;
    if (protoType === "Vless") {
      const vlessCfg: VlessConfig = {
        uuid: uuid || "b831381d-6324-4d53-ad4f-8cda98b3f811",
        flow: vlessFlow.trim() || undefined,
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
        mtu: mtuVal,
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
        mtu: mtuVal,
      };
    }

    await onAdd(profile);
    setCustomMtu("");
    setShowManualModal(false);
  };

  const handleStartEdit = (p: Profile) => {
    setEditingProfile(p);
    setEditName(p.name);
    setEditServer(p.server);
    setEditPort(p.port);
    setEditError(null);

    if (p.protocol.type === "Vless") {
      const cfg = p.protocol as VlessConfig;
      setEditUuid(cfg.uuid || "");
      setEditFlow(cfg.flow || "");
      setEditSni(cfg.sni || "");
      setEditFingerprint(cfg.fingerprint || "chrome");
      setEditNetwork(cfg.network || "tcp");
      setEditTls(cfg.tls ?? true);
      setEditReality(cfg.reality ?? false);
      setEditPublicKey(cfg.public_key || "");
      setEditShortId(cfg.short_id || "");
    } else if (p.protocol.type === "Shadowsocks") {
      const cfg = p.protocol as ShadowsocksConfig;
      setEditSsMethod(cfg.method || "chacha20-ietf-poly1305");
      setEditSsPassword(cfg.password || "");
    } else if (p.protocol.type === "Openvpn") {
      const cfg = p.protocol as OpenvpnConfig;
      setEditOvpnProto(cfg.proto || "udp");
      setEditOvpnUsername(cfg.username || "");
      setEditOvpnPassword(cfg.password || "");
    }
  };

  const handleSaveEdit = async () => {
    if (!editingProfile) return;

    const trimmedServer = editServer.trim();
    if (!trimmedServer) {
      setEditError("Адрес сервера не может быть пустым");
      return;
    }

    if (isNaN(editPort) || editPort < 1 || editPort > 65535) {
      setEditError("Порт должен быть числом от 1 до 65535");
      return;
    }

    const trimmedName = editName.trim() || `${trimmedServer}:${editPort}`;

    let updatedProtocol: Profile["protocol"];

    if (editingProfile.protocol.type === "Vless") {
      const trimmedUuid = editUuid.trim();
      if (!trimmedUuid) {
        setEditError("UUID не может быть пустым");
        return;
      }
      const uuidRegex = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
      if (!uuidRegex.test(trimmedUuid)) {
        setEditError("Неверный формат UUID (ожидается xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx)");
        return;
      }

      if (editReality && !editPublicKey.trim()) {
        setEditError("Для REALITY требуется указать публичный ключ (Public Key)");
        return;
      }

      const vlessCfg: VlessConfig = {
        uuid: trimmedUuid,
        flow: editFlow.trim() || undefined,
        network: editNetwork.trim() || "tcp",
        tls: editTls,
        sni: editSni.trim() || undefined,
        fingerprint: editFingerprint.trim() || undefined,
        reality: editReality,
        public_key: editReality ? (editPublicKey.trim() || undefined) : undefined,
        short_id: editReality ? (editShortId.trim() || undefined) : undefined,
      };

      updatedProtocol = { type: "Vless", ...vlessCfg };
    } else if (editingProfile.protocol.type === "Shadowsocks") {
      if (!editSsPassword.trim()) {
        setEditError("Пароль не может быть пустым");
        return;
      }
      const ssCfg: ShadowsocksConfig = {
        method: editSsMethod,
        password: editSsPassword.trim(),
      };
      updatedProtocol = { type: "Shadowsocks", ...ssCfg };
    } else {
      const prevCfg = editingProfile.protocol as OpenvpnConfig;
      const ovpnCfg: OpenvpnConfig = {
        ...prevCfg,
        proto: editOvpnProto,
        username: editOvpnUsername.trim() || undefined,
        password: editOvpnPassword || undefined,
      };
      updatedProtocol = { type: "Openvpn", ...ovpnCfg };
    }

    const updatedProfile: Profile = {
      ...editingProfile,
      name: trimmedName,
      server: trimmedServer,
      port: editPort,
      protocol: updatedProtocol,
    };

    setIsSavingEdit(true);
    setEditError(null);
    try {
      if (onUpdate) {
        await onUpdate(updatedProfile);
      } else {
        await api.updateProfile(updatedProfile);
        if (onReload) await onReload();
      }
      setEditingProfile(null);
    } catch (err: any) {
      setEditError(err?.message || String(err));
    } finally {
      setIsSavingEdit(false);
    }
  };

  return (
    <div className="p-4 space-y-4">
      {/* Hidden file input for .ovpn */}
      <input
        type="file"
        ref={fileInputRef}
        accept=".ovpn"
        className="hidden"
        onChange={handleFileChange}
      />

      {/* Action Buttons */}
      <div className="flex gap-2">
        <button
          onClick={handleImportOvpnClick}
          className="flex-1 flex items-center justify-center gap-1.5 px-3 py-2 bg-amber-600 hover:bg-amber-500 text-white rounded-xl text-xs font-semibold shadow-lg shadow-amber-600/20 transition-all"
        >
          <FileText className="w-3.5 h-3.5" />
          Import .ovpn
        </button>
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
              Import an .ovpn file or VLESS / Shadowsocks link to get started
            </p>
          </div>
        ) : (
          profiles.map((p) => {
            const isActive = p.id === activeProfileId;
            const isVless = p.protocol.type === "Vless";
            const isOpenvpn = p.protocol.type === "Openvpn";
            const isReality = isVless && (p.protocol as VlessConfig).reality;
            const ovpnProto = isOpenvpn
              ? (p.protocol as OpenvpnConfig).proto?.toUpperCase()
              : "";
            const ovpnCipher = isOpenvpn ? (p.protocol as OpenvpnConfig).cipher : null;

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
                    <div className="flex items-center gap-1.5 flex-wrap">
                      <p className="text-xs font-semibold text-slate-200 truncate">
                        {p.name}
                      </p>
                      {isOpenvpn ? (
                        <span className="px-1.5 py-0.2 text-[9px] font-mono rounded bg-amber-950 text-amber-400 border border-amber-800/50">
                          OPENVPN {ovpnProto}
                        </span>
                      ) : (
                        <span className="px-1.5 py-0.2 text-[9px] font-mono rounded bg-slate-800 text-slate-400 border border-slate-700">
                          {p.protocol.type.toUpperCase()}
                        </span>
                      )}
                      {isReality && (
                        <span className="px-1.5 py-0.2 text-[9px] font-mono rounded bg-emerald-950 text-emerald-400 border border-emerald-800/50">
                          REALITY
                        </span>
                      )}
                      {isVless && (p.protocol as VlessConfig).flow && (
                        <span className="px-1.5 py-0.2 text-[9px] font-mono rounded bg-orange-950/60 text-orange-400 border border-orange-800/40">
                          {(p.protocol as VlessConfig).flow}
                        </span>
                      )}
                      {ovpnCipher && (
                        <span className="px-1.5 py-0.2 text-[9px] font-mono rounded bg-slate-800/80 text-slate-400 border border-slate-700/60">
                          {ovpnCipher}
                        </span>
                      )}
                      {p.mtu && (
                        <span className="px-1.5 py-0.2 text-[9px] font-mono rounded bg-slate-800/80 text-orange-400 border border-slate-700/60">
                          MTU {p.mtu}
                        </span>
                      )}
                      {p.only_network_resources && (
                        <span className="px-1.5 py-0.2 text-[9px] font-semibold rounded bg-emerald-950 text-emerald-300 border border-emerald-800/60">
                          ТОЛЬКО РЕСУРСЫ СЕТИ
                        </span>
                      )}
                      {isOpenvpn && !(p.protocol as OpenvpnConfig).ca_cert && !(p.protocol as OpenvpnConfig).tls_auth && !(p.protocol as OpenvpnConfig).tls_crypt && (
                        <span className="px-1.5 py-0.2 text-[9px] font-semibold rounded bg-rose-950 text-rose-300 border border-rose-800/60" title="Требуется CA сертификат. Переимпортируйте профиль через кнопку Import .ovpn">
                          НЕТ CA СЕРТИФИКАТА
                        </span>
                      )}
                    </div>
                    <p className="text-[11px] text-slate-400 truncate mt-0.5">
                      {p.server}:{p.port}
                    </p>
                  </div>
                </div>

                <div className="flex items-center gap-2 shrink-0">
                  <label
                    onClick={(e) => e.stopPropagation()}
                    className="flex items-center gap-1.5 text-[11px] text-slate-400 hover:text-slate-200 cursor-pointer select-none px-2 py-1 rounded-lg bg-slate-800/60 hover:bg-slate-800 border border-slate-700/50"
                    title="Использовать только для доступа к ресурсам из hosts и корпоративной сети (остальной интернет напрямую или через основной VPN)"
                  >
                    <input
                      type="checkbox"
                      checked={!!p.only_network_resources}
                      onChange={async (e) => {
                        await api.toggleProfileNetworkOnly(p.id, e.target.checked);
                        if (onReload) await onReload();
                      }}
                      className="rounded bg-slate-950 border-slate-700 text-emerald-500 focus:ring-0 cursor-pointer w-3.5 h-3.5"
                    />
                    <span>Только сеть</span>
                  </label>

                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      handleStartEdit(p);
                    }}
                    className="p-1.5 text-slate-400 hover:text-orange-400 transition-colors rounded-lg hover:bg-slate-800"
                    title="Редактировать профиль"
                  >
                    <Pencil className="w-3.5 h-3.5" />
                  </button>

                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      onDelete(p.id);
                    }}
                    className="p-1.5 text-slate-500 hover:text-rose-400 transition-colors rounded-lg hover:bg-slate-800"
                    title="Удалить профиль"
                  >
                    <Trash2 className="w-3.5 h-3.5" />
                  </button>
                </div>
              </div>
            );
          })
        )}
      </div>

      {/* OpenVPN Credentials / Options Modal */}
      {showOvpnModal && (
        <div className="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4">
          <div className="bg-slate-900 border border-slate-800 rounded-2xl w-full max-w-md p-5 space-y-4 shadow-2xl">
            <h3 className="text-sm font-bold text-white flex items-center gap-2">
              <Lock className="w-4 h-4 text-amber-400" />
              OpenVPN Configuration & Credentials
            </h3>

            {ovpnFilePath ? (
              <p className="text-xs text-slate-400 font-mono bg-slate-950 p-2 rounded border border-slate-800 truncate">
                File: {ovpnFilePath}
              </p>
            ) : null}

            <div className="space-y-3 text-xs">
              <div>
                <label className="block text-slate-400 mb-1">
                  Username (optional / if required by server)
                </label>
                <input
                  type="text"
                  value={ovpnUsername}
                  onChange={(e) => setOvpnUsername(e.target.value)}
                  placeholder="Username"
                  className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-slate-200"
                />
              </div>

              <div>
                <label className="block text-slate-400 mb-1">
                  Password / Passphrase (optional)
                </label>
                <input
                  type="password"
                  value={ovpnPassword}
                  onChange={(e) => setOvpnPassword(e.target.value)}
                  placeholder="Password"
                  className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-slate-200"
                />
              </div>

              {ovpnContent ? (
                <div>
                  <label className="block text-slate-400 mb-1">OVPN Content</label>
                  <textarea
                    rows={4}
                    value={ovpnContent}
                    onChange={(e) => setOvpnContent(e.target.value)}
                    className="w-full bg-slate-950 border border-slate-800 rounded-xl p-3 text-xs text-slate-200 font-mono"
                  />
                </div>
              ) : null}
            </div>

            {ovpnError && (
              <p className="text-xs text-rose-400 bg-rose-950/40 p-2.5 rounded-lg border border-rose-800/50">
                {ovpnError}
              </p>
            )}

            <div className="flex justify-end gap-2 pt-2">
              <button
                onClick={() => setShowOvpnModal(false)}
                className="px-3 py-1.5 text-xs text-slate-400 hover:text-slate-200"
              >
                Cancel
              </button>
              <button
                onClick={handleOvpnSubmit}
                className="px-4 py-1.5 bg-amber-600 hover:bg-amber-500 text-white text-xs font-semibold rounded-xl"
              >
                Save Profile
              </button>
            </div>
          </div>
        </div>
      )}

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
                <label className="block text-slate-400 mb-1">MTU (Optional, default 1500)</label>
                <input
                  type="number"
                  min={576}
                  max={65535}
                  value={customMtu}
                  onChange={(e) => setCustomMtu(e.target.value)}
                  placeholder="1500"
                  className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-slate-200"
                />
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
                    <div className="flex items-center justify-between mb-1">
                      <label className="text-slate-400">Flow</label>
                      <div className="flex gap-1">
                        <button
                          type="button"
                          onClick={() => setVlessFlow("xtls-rprx-vision")}
                          className={`text-[10px] px-1.5 py-0.5 rounded transition-all ${
                            vlessFlow === "xtls-rprx-vision"
                              ? "bg-orange-600/30 text-orange-300 border border-orange-500/50"
                              : "bg-slate-800 text-slate-400 hover:text-slate-200 border border-slate-700/50"
                          }`}
                        >
                          xtls-rprx-vision
                        </button>
                        <button
                          type="button"
                          onClick={() => setVlessFlow("")}
                          className={`text-[10px] px-1.5 py-0.5 rounded transition-all ${
                            !vlessFlow
                              ? "bg-slate-700 text-slate-200 border border-slate-600"
                              : "bg-slate-800 text-slate-400 hover:text-slate-200 border border-slate-700/50"
                          }`}
                        >
                          None
                        </button>
                      </div>
                    </div>
                    <input
                      type="text"
                      value={vlessFlow}
                      onChange={(e) => setVlessFlow(e.target.value)}
                      placeholder="xtls-rprx-vision или оставьте пустым"
                      className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-slate-200 font-mono text-[11px]"
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

      {/* Edit Profile Modal */}
      {editingProfile && (
        <div className="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4">
          <div className="bg-slate-900 border border-slate-800 rounded-2xl w-full max-w-lg p-5 space-y-4 shadow-2xl max-h-[90vh] overflow-y-auto">
            <div className="flex items-center justify-between border-b border-slate-800/80 pb-3">
              <h3 className="text-sm font-bold text-white flex items-center gap-2">
                <Pencil className="w-4 h-4 text-orange-400" />
                <span>Редактирование профиля</span>
                <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-slate-800 text-slate-400 border border-slate-700 uppercase">
                  {editingProfile.protocol.type}
                </span>
              </h3>
              <button
                onClick={() => setEditingProfile(null)}
                className="p-1 text-slate-400 hover:text-slate-200 rounded-lg hover:bg-slate-800 transition-colors"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            <div className="space-y-3 text-xs">
              {/* Profile Name */}
              <div>
                <label className="block text-slate-400 mb-1">Название профиля</label>
                <input
                  type="text"
                  value={editName}
                  onChange={(e) => setEditName(e.target.value)}
                  placeholder="My Server"
                  className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-slate-200 focus:outline-none focus:border-orange-500"
                />
              </div>

              {/* Server Host / IP Address and Port */}
              <div className="grid grid-cols-3 gap-2">
                <div className="col-span-2">
                  <label className="block text-slate-400 mb-1">
                    Адрес сервера / IP <span className="text-orange-400">*</span>
                  </label>
                  <input
                    type="text"
                    value={editServer}
                    onChange={(e) => setEditServer(e.target.value)}
                    placeholder="example.com или 192.168.1.1"
                    className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-slate-200 font-mono text-[11px] focus:outline-none focus:border-orange-500"
                  />
                </div>
                <div>
                  <label className="block text-slate-400 mb-1">
                    Порт <span className="text-orange-400">*</span>
                  </label>
                  <input
                    type="number"
                    value={editPort}
                    onChange={(e) => setEditPort(parseInt(e.target.value) || 0)}
                    min={1}
                    max={65535}
                    className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-slate-200 font-mono text-[11px] focus:outline-none focus:border-orange-500"
                  />
                </div>
              </div>

              {/* Protocol specific settings */}
              {editingProfile.protocol.type === "Vless" && (
                <>
                  {/* Flow control */}
                  <div>
                    <div className="flex items-center justify-between mb-1">
                      <label className="text-slate-400 font-medium">Flow (управление потоком XTLS)</label>
                      <div className="flex gap-1">
                        <button
                          type="button"
                          onClick={() => setEditFlow("xtls-rprx-vision")}
                          className={`text-[10px] px-2 py-0.5 rounded transition-all ${
                            editFlow === "xtls-rprx-vision"
                              ? "bg-orange-600/30 text-orange-300 border border-orange-500/50"
                              : "bg-slate-800 text-slate-400 hover:text-slate-200 border border-slate-700/50"
                          }`}
                        >
                          xtls-rprx-vision
                        </button>
                        <button
                          type="button"
                          onClick={() => setEditFlow("")}
                          className={`text-[10px] px-2 py-0.5 rounded transition-all ${
                            !editFlow
                              ? "bg-slate-700 text-slate-200 border border-slate-600"
                              : "bg-slate-800 text-slate-400 hover:text-slate-200 border border-slate-700/50"
                          }`}
                        >
                          Без flow (пусто)
                        </button>
                      </div>
                    </div>
                    <input
                      type="text"
                      value={editFlow}
                      onChange={(e) => setEditFlow(e.target.value)}
                      placeholder="xtls-rprx-vision или оставьте пустым"
                      className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-slate-200 font-mono text-[11px] focus:outline-none focus:border-orange-500"
                    />
                    <p className="text-[10px] text-slate-500 mt-1">
                      Для протокола VLESS Reality рекомендуется xtls-rprx-vision. Оставьте пустым, если flow не требуется.
                    </p>
                  </div>

                  {/* UUID */}
                  <div>
                    <label className="block text-slate-400 mb-1">
                      UUID пользователя <span className="text-orange-400">*</span>
                    </label>
                    <input
                      type="text"
                      value={editUuid}
                      onChange={(e) => setEditUuid(e.target.value)}
                      placeholder="b831381d-6324-4d53-ad4f-8cda98b3f811"
                      className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-slate-200 font-mono text-[11px] focus:outline-none focus:border-orange-500"
                    />
                  </div>

                  {/* SNI / Server Name */}
                  <div>
                    <label className="block text-slate-400 mb-1">SNI (Server Name Indication)</label>
                    <input
                      type="text"
                      value={editSni}
                      onChange={(e) => setEditSni(e.target.value)}
                      placeholder="www.microsoft.com"
                      className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-slate-200 focus:outline-none focus:border-orange-500"
                    />
                  </div>

                  {/* Network Transport & Fingerprint */}
                  <div className="grid grid-cols-2 gap-2">
                    <div>
                      <label className="block text-slate-400 mb-1">Транспорт (Network)</label>
                      <select
                        value={editNetwork}
                        onChange={(e) => setEditNetwork(e.target.value)}
                        className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-slate-200 focus:outline-none focus:border-orange-500"
                      >
                        <option value="tcp">TCP</option>
                        <option value="ws">WebSocket (WS)</option>
                        <option value="grpc">gRPC</option>
                      </select>
                    </div>

                    <div>
                      <label className="block text-slate-400 mb-1">Fingerprint (uTLS)</label>
                      <select
                        value={editFingerprint}
                        onChange={(e) => setEditFingerprint(e.target.value)}
                        className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-slate-200 focus:outline-none focus:border-orange-500"
                      >
                        <option value="chrome">Chrome</option>
                        <option value="firefox">Firefox</option>
                        <option value="safari">Safari</option>
                        <option value="edge">Edge</option>
                        <option value="ios">iOS</option>
                        <option value="android">Android</option>
                        <option value="randomized">Randomized</option>
                      </select>
                    </div>
                  </div>

                  {/* TLS & Reality Toggles */}
                  <div className="flex gap-4 pt-1">
                    <label className="flex items-center gap-2 cursor-pointer select-none text-slate-300">
                      <input
                        type="checkbox"
                        checked={editTls}
                        onChange={(e) => setEditTls(e.target.checked)}
                        className="rounded bg-slate-950 border-slate-700 text-orange-500 focus:ring-0 cursor-pointer"
                      />
                      <span>Включить TLS</span>
                    </label>

                    <label className="flex items-center gap-2 cursor-pointer select-none text-slate-300">
                      <input
                        type="checkbox"
                        checked={editReality}
                        onChange={(e) => {
                          setEditReality(e.target.checked);
                          if (e.target.checked) setEditTls(true);
                        }}
                        className="rounded bg-slate-950 border-slate-700 text-emerald-500 focus:ring-0 cursor-pointer"
                      />
                      <span className="text-emerald-400 font-medium">REALITY</span>
                    </label>
                  </div>

                  {/* Reality specific fields */}
                  {editReality && (
                    <div className="space-y-2 p-3 rounded-xl bg-slate-950/60 border border-slate-800/80">
                      <div>
                        <label className="block text-slate-400 mb-1">
                          REALITY Public Key (pbk) <span className="text-emerald-400">*</span>
                        </label>
                        <input
                          type="text"
                          value={editPublicKey}
                          onChange={(e) => setEditPublicKey(e.target.value)}
                          placeholder="Публичный ключ REALITY"
                          className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-slate-200 font-mono text-[11px] focus:outline-none focus:border-emerald-500"
                        />
                      </div>

                      <div>
                        <label className="block text-slate-400 mb-1">Short ID (sid)</label>
                        <input
                          type="text"
                          value={editShortId}
                          onChange={(e) => setEditShortId(e.target.value)}
                          placeholder="Короткий ID (опционально)"
                          className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-slate-200 font-mono text-[11px] focus:outline-none focus:border-emerald-500"
                        />
                      </div>
                    </div>
                  )}
                </>
              )}

              {/* Shadowsocks settings */}
              {editingProfile.protocol.type === "Shadowsocks" && (
                <>
                  <div>
                    <label className="block text-slate-400 mb-1">Метод шифрования (Cipher)</label>
                    <select
                      value={editSsMethod}
                      onChange={(e) => setEditSsMethod(e.target.value)}
                      className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-slate-200 focus:outline-none focus:border-orange-500"
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
                    <label className="block text-slate-400 mb-1">Пароль (Password)</label>
                    <input
                      type="password"
                      value={editSsPassword}
                      onChange={(e) => setEditSsPassword(e.target.value)}
                      placeholder="Пароль"
                      className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-slate-200 focus:outline-none focus:border-orange-500"
                    />
                  </div>
                </>
              )}

              {/* OpenVPN settings */}
              {editingProfile.protocol.type === "Openvpn" && (
                <>
                  <div>
                    <label className="block text-slate-400 mb-1">Протокол OpenVPN</label>
                    <select
                      value={editOvpnProto}
                      onChange={(e) => setEditOvpnProto(e.target.value)}
                      className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-slate-200 focus:outline-none focus:border-orange-500"
                    >
                      <option value="udp">UDP</option>
                      <option value="tcp">TCP</option>
                    </select>
                  </div>

                  <div>
                    <label className="block text-slate-400 mb-1">Имя пользователя (опционально)</label>
                    <input
                      type="text"
                      value={editOvpnUsername}
                      onChange={(e) => setEditOvpnUsername(e.target.value)}
                      placeholder="Username"
                      className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-slate-200 focus:outline-none focus:border-orange-500"
                    />
                  </div>

                  <div>
                    <label className="block text-slate-400 mb-1">Пароль (опционально)</label>
                    <input
                      type="password"
                      value={editOvpnPassword}
                      onChange={(e) => setEditOvpnPassword(e.target.value)}
                      placeholder="Password"
                      className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-slate-200 focus:outline-none focus:border-orange-500"
                    />
                  </div>
                </>
              )}
            </div>

            {/* Error Message */}
            {editError && (
              <p className="text-xs text-rose-400 bg-rose-950/40 p-2.5 rounded-lg border border-rose-800/50">
                {editError}
              </p>
            )}

            {/* Footer Buttons */}
            <div className="flex justify-end gap-2 pt-2 border-t border-slate-800/80">
              <button
                type="button"
                onClick={() => setEditingProfile(null)}
                className="px-3.5 py-1.5 text-xs text-slate-400 hover:text-slate-200 transition-colors"
              >
                Отмена
              </button>
              <button
                type="button"
                onClick={handleSaveEdit}
                disabled={isSavingEdit || !editServer.trim()}
                className="px-4 py-1.5 bg-orange-600 hover:bg-orange-500 disabled:opacity-50 text-white text-xs font-semibold rounded-xl shadow-lg shadow-orange-600/20 transition-all"
              >
                {isSavingEdit ? "Сохранение..." : "Сохранить"}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
