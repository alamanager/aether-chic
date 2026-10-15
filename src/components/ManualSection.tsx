import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import jsqr from "jsqr";
import { ChevronDown, Plus, Trash2, Zap } from "lucide-react";
import { cn } from "@/lib/utils";
import { useConnectionStore, type ManualConfig } from "@/state/connectionStore";

interface ImportDto {
  kind: string;
  name: string;
  engine: string;
  outbound?: unknown;
  raw_json?: unknown;
  socks_port: number;
  host: string;
  port: number;
  reason: string;
}

function newId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) return crypto.randomUUID();
  return `${Date.now()}-${Math.floor(Math.random() * 1e9)}`;
}

function fmtMs(ms: number | null | undefined): string {
  if (ms === null || ms === undefined) return "—";
  return ms < 1000 ? `${ms}ms` : `${(ms / 1000).toFixed(1)}s`;
}

async function decodeQrImage(file: File): Promise<string | null> {
  const bmp = await createImageBitmap(file);
  const scale = Math.min(1, 1200 / Math.max(bmp.width, bmp.height));
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.round(bmp.width * scale));
  canvas.height = Math.max(1, Math.round(bmp.height * scale));
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx) return null;
  ctx.drawImage(bmp, 0, 0, canvas.width, canvas.height);
  const img = ctx.getImageData(0, 0, canvas.width, canvas.height);
  return jsqr(img.data, img.width, img.height)?.data ?? null;
}

/** Auto/Manual segmented switch (Pulse + Tune share it). */
export function ModeSwitch() {
  const mode = useConnectionStore((s) => s.manualMode);
  const setMode = useConnectionStore((s) => s.setManualMode);
  return (
    <div className="glass flex items-center gap-1 rounded-full p-1" role="group" aria-label="Connection mode">
      {(["auto", "manual"] as const).map((m) => {
        const active = (m === "manual") === mode;
        return (
          <button
            key={m}
            type="button"
            onClick={() => setMode(m === "manual")}
            aria-pressed={active}
            className={cn(
              "rounded-full px-4 py-1 text-[11px] font-bold tracking-wider uppercase transition-all outline-none focus-visible:ring-2 focus-visible:ring-primary",
              active ? "text-white shadow-[0_4px_24px_-4px_rgb(139_92_246/0.7)]" : "text-muted-foreground hover:text-foreground",
            )}
            style={active ? { background: "linear-gradient(135deg, #22d3ee, #818cf8 55%, #a855f7)" } : undefined}
          >
            {m === "auto" ? "Auto" : "Manual"}
          </button>
        );
      })}
    </div>
  );
}

function toConfig(dto: ImportDto, source: string): ManualConfig | null {
  if (dto.kind === "skipped") return null;
  if (dto.kind === "rawjson") {
    return {
      id: newId(),
      name: dto.name || "json",
      engine: dto.engine === "xray" ? "xray" : "singbox",
      rawJson: dto.raw_json,
      socksPort: dto.socks_port || 1870,
      host: "",
      port: 0,
      source,
    };
  }
  const engine =
    dto.engine === "xray" ? "xray" : dto.engine === "ssh" ? "ssh" : dto.engine === "ikev2" ? "ikev2" : "singbox";
  const params = (dto.outbound ?? {}) as Record<string, unknown>;
  const str = (v: unknown): string => (typeof v === "string" ? v : "");
  return {
    id: newId(),
    name: dto.name || dto.host,
    engine,
    outbound: dto.outbound,
    socksPort: 1870,
    host: dto.host,
    port: dto.port,
    source,
    username: engine === "ssh" || engine === "ikev2" ? str(params.user) || undefined : undefined,
    secret: engine === "ssh" ? str(params.key) || undefined : engine === "ikev2" ? str(params.pass) || undefined : undefined,
  };
}

/** Full manual-config manager: import (links/JSON/sub/file/QR), latency
 * ranking, active pick, sing-box live stats. Lives in Tune. */
export function ManualSection() {
  const configs = useConnectionStore((s) => s.configs);
  const activeId = useConnectionStore((s) => s.activeConfigId);
  const addConfigs = useConnectionStore((s) => s.addConfigs);
  const removeConfig = useConnectionStore((s) => s.removeConfig);
  const setActive = useConnectionStore((s) => s.setActiveConfig);
  const setVia = useConnectionStore((s) => s.setConfigVia);
  const setLatency = useConnectionStore((s) => s.setConfigLatency);
  const connected = useConnectionStore((s) => s.status.state === "Connected");
  const [paste, setPaste] = useState("");
  const [subUrl, setSubUrl] = useState("");
  const [notice, setNotice] = useState<string | null>(null);
  const [testing, setTesting] = useState(false);
  const [clashCount, setClashCount] = useState<number | null>(null);
  const [open, setOpen] = useState(false);

  const active = configs.find((c) => c.id === activeId) ?? null;

  const ingest = useCallback(
    (dtos: ImportDto[], source: string) => {
      const cfgs = dtos.map((d) => toConfig(d, source)).filter((c): c is ManualConfig => c !== null);
      const skipped = dtos.filter((d) => d.kind === "skipped");
      if (cfgs.length > 0) addConfigs(cfgs);
      setNotice(
        skipped.length > 0
          ? `${cfgs.length} added, ${skipped.length} skipped — ${skipped[0].reason}`.slice(0, 140)
          : `${cfgs.length} added.`,
      );
    },
    [addConfigs],
  );

  const importPaste = async () => {
    if (!paste.trim()) return;
    try {
      const dtos = await invoke<ImportDto[]>("manual_import_text", { text: paste });
      ingest(dtos, "paste");
      setPaste("");
    } catch (e) {
      setNotice(`Import failed: ${String(e).slice(0, 120)}`);
    }
  };

  const importSub = async () => {
    if (!subUrl.trim()) return;
    setNotice("Fetching subscription…");
    try {
      const dtos = await invoke<ImportDto[]>("manual_import_sub", { url: subUrl.trim() });
      ingest(dtos, "sub");
    } catch (e) {
      setNotice(`Subscription failed: ${String(e).slice(0, 120)}`);
    }
  };

  const importFile = async (file: File) => {
    try {
      if (/\.(png|jpe?g|webp|bmp)$/i.test(file.name)) {
        const text = await decodeQrImage(file);
        if (!text) {
          setNotice("No QR code found in that image.");
          return;
        }
        const dtos = await invoke<ImportDto[]>("manual_import_text", { text });
        ingest(dtos, "qr");
        return;
      }
      const text = await file.text();
      if (/\.ovpn$/i.test(file.name)) {
        const dto = await invoke<ImportDto>("manual_import_ovpn", { text });
        ingest([dto], "file");
      } else {
        const dtos = await invoke<ImportDto[]>("manual_import_text", { text });
        ingest(dtos, "file");
      }
    } catch (e) {
      setNotice(`File failed: ${String(e).slice(0, 120)}`);
    }
  };

  const autoPick = async () => {
    await testAll();
    const s = useConnectionStore.getState();
    const ranked = s.configs
      .filter((c) => c.latencyMs !== null && c.latencyMs !== undefined)
      .sort((a, b) => (a.latencyMs ?? Infinity) - (b.latencyMs ?? Infinity));
    if (ranked.length > 0) {
      s.setActiveConfig(ranked[0].id);
      setNotice(`Fastest: ${ranked[0].name} (${fmtMs(ranked[0].latencyMs ?? null)}). Hit connect.`);
    } else {
      setNotice("Nothing answered the ping — check the list.");
    }
  };

  const testAll = async () => {
    const testable = configs.filter((c) => c.host && c.port > 0);
    if (testable.length === 0) return;
    setTesting(true);
    await Promise.all(
      testable.map(async (c) => {
        try {
          // -1 sentinel would read as "untested"; null clears instead.
          const ms = await invoke<number>("tcp_ping", { host: c.host, port: c.port });
          setLatency(c.id, ms);
        } catch {
          setLatency(c.id, null);
        }
      }),
    );
    setTesting(false);
  };

  // sing-box Clash API: live connection count, no Rust involved.
  // Shown only while connected; stale counts hide with the section.
  useEffect(() => {
    if (!connected || active?.engine !== "singbox") return;
    let dead = false;
    const poll = async () => {
      try {
        const r = await fetch("http://127.0.0.1:1872/connections");
        if (!r.ok) return;
        const j = (await r.json()) as { connections?: unknown[] };
        if (!dead && Array.isArray(j.connections)) setClashCount(j.connections.length);
      } catch {
        /* xray / api off — silent */
      }
    };
    void poll();
    const id = setInterval(poll, 5000);
    return () => {
      dead = true;
      clearInterval(id);
    };
  }, [connected, active?.engine]);

  return (
    <div className="flex flex-col gap-2">
      <div className="flex flex-col gap-1.5">
        <textarea
          value={paste}
          onChange={(e) => setPaste(e.target.value)}
          placeholder="Paste link(s) or JSON…"
          rows={2}
          dir="ltr"
          className="w-full rounded-xl bg-black/20 px-3 py-2 font-mono text-xs text-foreground ring-1 ring-white/10 outline-none placeholder:text-muted-foreground/50 focus:ring-primary light:bg-black/5 light:ring-black/10"
        />
        <div className="flex gap-2">
          <button
            type="button"
            onClick={() => void importPaste()}
            className="flex flex-1 items-center justify-center gap-1 rounded-xl bg-surface-3 px-2 py-1.5 text-xs font-bold text-foreground transition-colors hover:bg-surface-4"
          >
            <Plus className="size-3.5" /> Add
          </button>
          <label className="flex flex-1 cursor-pointer items-center justify-center gap-1 rounded-xl bg-surface-3 px-2 py-1.5 text-xs font-bold text-foreground transition-colors hover:bg-surface-4">
            File / QR
            <input
              type="file"
              className="hidden"
              accept=".json,.txt,.ovpn,.conf,.png,.jpg,.jpeg,.webp,.bmp"
              onChange={(e) => {
                const f = e.target.files?.[0];
                if (f) void importFile(f);
                e.target.value = "";
              }}
            />
          </label>
        </div>
        <div className="flex gap-2">
          <input
            value={subUrl}
            onChange={(e) => setSubUrl(e.target.value)}
            placeholder="Subscription URL…"
            dir="ltr"
            className="min-w-0 flex-1 rounded-xl bg-black/20 px-3 py-1.5 font-mono text-xs text-foreground ring-1 ring-white/10 outline-none placeholder:text-muted-foreground/50 focus:ring-primary light:bg-black/5 light:ring-black/10"
          />
          <button
            type="button"
            onClick={() => void importSub()}
            className="shrink-0 rounded-xl bg-surface-3 px-3 py-1.5 text-xs font-bold text-foreground transition-colors hover:bg-surface-4"
          >
            Fetch
          </button>
        </div>
        {notice && <p className="px-1 text-[11px] text-muted-foreground">{notice}</p>}
      </div>
      <SshForm />
      <Ikev2Form />
      <VaultBox />
      <div className="flex items-center justify-between px-1">
        <span className="text-[11px] font-bold text-muted-foreground">
          {configs.length === 0 ? "No configs yet" : `${configs.length} configs`}
          {connected && active?.engine === "singbox" && clashCount !== null && ` · ${clashCount} live via engine`}
        </span>
        <div className="flex gap-2">
          <button
            type="button"
            onClick={() => void testAll()}
            disabled={testing || configs.length === 0}
            className="flex flex-1 items-center justify-center gap-1 rounded-lg bg-surface-3 px-2 py-1 text-[11px] font-bold text-foreground transition-colors hover:bg-surface-4 disabled:opacity-50"
          >
            <Zap className="size-3" /> {testing ? "Pinging…" : "Test all"}
          </button>
          <button
            type="button"
            onClick={() => autoPick()}
            disabled={testing || configs.length === 0}
            title="Activate the fastest tested config"
            className="flex flex-1 items-center justify-center gap-1 rounded-lg bg-surface-3 px-2 py-1 text-[11px] font-bold text-foreground transition-colors hover:bg-surface-4 disabled:opacity-50"
          >
            Auto-pick
          </button>
        </div>
      </div>
      {configs.length > 0 && (
        <button
          type="button"
          onClick={() => setOpen((o) => !o)}
          className="flex items-center gap-1 px-1 text-[11px] text-muted-foreground"
        >
          <ChevronDown className={cn("size-3.5 transition-transform", open && "rotate-180")} />
          {open ? "Hide list" : "Show list"}
        </button>
      )}
      {open &&
        configs.map((c) => {
          const peers = configs.filter((o) => o.id !== c.id && o.engine === c.engine && (c.engine === "singbox" || c.engine === "xray"));
          return (
          <div
            key={c.id}
            className={cn(
              "flex flex-col gap-1 rounded-xl px-2 py-1.5 ring-1 transition-colors",
              activeId === c.id ? "bg-primary/10 ring-primary/40" : "bg-black/20 ring-white/10 light:bg-black/5 light:ring-black/10",
            )}
          >
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => setActive(c.id)}
              aria-label={`Use ${c.name}`}
              className={cn(
                "size-3.5 shrink-0 rounded-full ring-2",
                activeId === c.id ? "bg-status-connected ring-status-connected" : "ring-muted-foreground/40 hover:ring-foreground",
              )}
            />
            <span className="min-w-0 flex-1 text-left">
              <span className="block truncate text-xs font-semibold text-foreground">{c.name}</span>
              <span className="block truncate font-mono text-[10px] text-muted-foreground" dir="ltr">
                {c.engine} · {c.host || "json"}:{c.port || "—"} · {c.source}
              </span>
            </span>
            <span
              className={cn(
                "shrink-0 rounded-full px-2 py-0.5 font-mono text-[10px] font-bold",
                c.latencyMs === null || c.latencyMs === undefined
                  ? "bg-surface-3 text-muted-foreground"
                  : c.latencyMs < 800
                    ? "bg-status-connected/15 text-status-connected"
                    : "bg-status-error/15 text-status-error",
              )}
              dir="ltr"
            >
              {fmtMs(c.latencyMs ?? null)}
            </span>
            <button
              type="button"
              onClick={() => removeConfig(c.id)}
              aria-label={`Delete ${c.name}`}
              className="grid size-7 shrink-0 place-items-center rounded-lg text-muted-foreground transition-colors hover:bg-surface-3 hover:text-foreground"
            >
              <Trash2 className="size-3.5" />
            </button>
          </div>
          {peers.length > 0 && (
            <label className="flex items-center gap-1.5 px-1 text-[10px] text-muted-foreground">
              via
              <select
                value={c.via ?? ""}
                onChange={(e) => setVia(c.id, e.target.value || null)}
                aria-label={`Chain ${c.name} through`}
                className="min-w-0 flex-1 rounded-md bg-black/20 px-1 py-0.5 font-mono text-[10px] text-foreground ring-1 ring-white/10 outline-none light:bg-black/5 light:ring-black/10"
              >
                <option value="">direct</option>
                {peers.map((o) => (
                  <option key={o.id} value={o.id}>
                    {o.name}
                  </option>
                ))}
              </select>
            </label>
          )}
          </div>
          );
        })}
    </div>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="flex min-w-0 flex-1 flex-col gap-0.5">
      <span className="text-[10px] font-medium tracking-wide text-muted-foreground uppercase">{label}</span>
      {children}
    </label>
  );
}

const inputCls =
  "w-full rounded-lg bg-black/20 px-2 py-1.5 font-mono text-xs text-foreground ring-1 ring-white/10 outline-none placeholder:text-muted-foreground/50 focus:ring-primary light:bg-black/5 light:ring-black/10";

/** Key-based SSH (system client). Passwords cannot go through headless. */
function SshForm() {
  const addConfigs = useConnectionStore((s) => s.addConfigs);
  const [host, setHost] = useState("");
  const [port, setPort] = useState("22");
  const [user, setUser] = useState("");
  const [key, setKey] = useState("");
  const [open, setOpen] = useState(false);
  if (!open) {
    return (
      <button type="button" onClick={() => setOpen(true)} className="px-1 text-left text-[11px] font-bold text-muted-foreground hover:text-foreground">
        + SSH server (key)
      </button>
    );
  }
  return (
    <div className="flex flex-col gap-1.5 rounded-xl bg-black/20 p-2 ring-1 ring-white/10 light:bg-black/5 light:ring-black/10">
      <div className="flex gap-1.5">
        <Field label="Host"><input className={inputCls} value={host} onChange={(e) => setHost(e.target.value)} dir="ltr" /></Field>
        <Field label="Port"><input className={inputCls} value={port} onChange={(e) => setPort(e.target.value.replace(/\D/g, ""))} dir="ltr" /></Field>
        <Field label="User"><input className={inputCls} value={user} onChange={(e) => setUser(e.target.value)} dir="ltr" /></Field>
      </div>
      <Field label="Private key text">
        <textarea className={inputCls} rows={2} value={key} onChange={(e) => setKey(e.target.value)} dir="ltr" placeholder="-----BEGIN OPENSSH PRIVATE KEY-----" />
      </Field>
      <div className="flex gap-2">
        <button
          type="button"
          onClick={() => {
            const p = Number(port) || 22;
            if (!host.trim() || !user.trim() || !key.trim()) return;
            addConfigs([{
              id: newId(), name: `${user}@${host.trim()}`, engine: "ssh" as const,
              socksPort: 1870, host: host.trim(), port: p, source: "form",
              username: user.trim(), secret: key.trim(),
            }]);
            setHost(""); setPort("22"); setUser(""); setKey(""); setOpen(false);
          }}
          className="flex-1 rounded-lg bg-surface-3 px-2 py-1.5 text-xs font-bold text-foreground transition-colors hover:bg-surface-4"
        >
          Add SSH
        </button>
        <button type="button" onClick={() => setOpen(false)} className="rounded-lg px-2 py-1.5 text-xs text-muted-foreground hover:text-foreground">
          Close
        </button>
      </div>
    </div>
  );
}

/** IKEv2 via the OS client (Windows). EAP user/pass; needs admin. */
function Ikev2Form() {
  const addConfigs = useConnectionStore((s) => s.addConfigs);
  const [server, setServer] = useState("");
  const [user, setUser] = useState("");
  const [pass, setPass] = useState("");
  const [name, setName] = useState("");
  const [open, setOpen] = useState(false);
  if (!open) {
    return (
      <button type="button" onClick={() => setOpen(true)} className="px-1 text-left text-[11px] font-bold text-muted-foreground hover:text-foreground">
        + IKEv2 server (Windows, admin)
      </button>
    );
  }
  return (
    <div className="flex flex-col gap-1.5 rounded-xl bg-black/20 p-2 ring-1 ring-white/10 light:bg-black/5 light:ring-black/10">
      <div className="flex gap-1.5">
        <Field label="Server"><input className={inputCls} value={server} onChange={(e) => setServer(e.target.value)} dir="ltr" /></Field>
        <Field label="Name"><input className={inputCls} value={name} onChange={(e) => setName(e.target.value)} dir="ltr" /></Field>
      </div>
      <div className="flex gap-1.5">
        <Field label="User"><input className={inputCls} value={user} onChange={(e) => setUser(e.target.value)} dir="ltr" /></Field>
        <Field label="Password"><input className={inputCls} type="password" value={pass} onChange={(e) => setPass(e.target.value)} dir="ltr" /></Field>
      </div>
      <p className="px-1 text-[10px] leading-4 text-muted-foreground">EAP user/pass only (no PSK/cert here). Needs Windows + admin.</p>
      <div className="flex gap-2">
        <button
          type="button"
          onClick={() => {
            if (!server.trim() || !user || !pass) return;
            addConfigs([{
              id: newId(), name: name.trim() || server.trim(), engine: "ikev2" as const,
              socksPort: 0, host: server.trim(), port: 500, source: "form",
              username: user, secret: pass,
            }]);
            setServer(""); setUser(""); setPass(""); setName(""); setOpen(false);
          }}
          className="flex-1 rounded-lg bg-surface-3 px-2 py-1.5 text-xs font-bold text-foreground transition-colors hover:bg-surface-4"
        >
          Add IKEv2
        </button>
        <button type="button" onClick={() => setOpen(false)} className="rounded-lg px-2 py-1.5 text-xs text-muted-foreground hover:text-foreground">
          Close
        </button>
      </div>
    </div>
  );
}

/** Vault share: seal one config to a password-locked expiring file, or open one. */
function VaultBox() {
  const configs = useConnectionStore((s) => s.configs);
  const addConfigs = useConnectionStore((s) => s.addConfigs);
  const [cfgId, setCfgId] = useState("");
  const [pass, setPass] = useState("");
  const [days, setDays] = useState("7");
  const [msg, setMsg] = useState<string | null>(null);

  const selected = configs.find((c) => c.id === cfgId) ?? configs[0];

  const exportVault = async () => {
    if (!selected) return;
    try {
      const payload = JSON.stringify({
        engine: selected.engine,
        outbound: selected.outbound ?? null,
        rawJson: selected.rawJson ?? null,
        host: selected.host, port: selected.port,
        username: selected.username ?? "", secret: selected.secret ?? "",
      });
      const blob = await invoke<string>("vault_seal", {
        configJson: payload, name: selected.name, password: pass, daysValid: Number(days) || 0,
      });
      const a = document.createElement("a");
      a.href = `data:application/octet-stream;base64,${blob}`;
      a.download = `${selected.name.replace(/[^\w-]+/g, "_")}.vault`;
      a.click();
      setMsg("Vault file saved � share it, not the password.");
    } catch (e) {
      setMsg(`Seal failed: ${String(e).slice(0, 120)}`);
    }
  };

  const importVault = async (file: File) => {
    try {
      const text = await file.text();
      const opened = await invoke<{ name: string; config_json: string; expired: boolean }>("vault_open", {
        blobB64: text.trim(), password: pass,
      });
      if (opened.expired) {
        setMsg("That vault expired � ask for a fresh one.");
        return;
      }
      const v = JSON.parse(opened.config_json) as Record<string, unknown>;
      const str = (x: unknown): string => (typeof x === "string" ? x : "");
      const num = (x: unknown): number => (typeof x === "number" ? x : 0);
      addConfigs([{
        id: newId(),
        name: String((v.name as string) ?? opened.name ?? "vault"),
        engine: v.engine === "xray" ? "xray" : "singbox",
        outbound: (v.outbound ?? undefined) as ManualConfig["outbound"],
        rawJson: (v.rawJson ?? undefined) as ManualConfig["rawJson"],
        socksPort: 1870, host: str(v.host), port: num(v.port), source: "vault",
        username: str(v.username) || undefined, secret: str(v.secret) || undefined,
      }]);
      setMsg(`Imported �${opened.name}�.`);
    } catch (e) {
      setMsg(`Open failed: ${String(e).slice(0, 120)}`);
    }
  };

  if (configs.length === 0) return null;
  return (
    <div className="flex flex-col gap-1.5 rounded-xl bg-black/20 p-2 ring-1 ring-white/10 light:bg-black/5 light:ring-black/10">
      <span className="px-1 text-[11px] font-bold text-muted-foreground">Vault share (password-locked file)</span>
      <div className="flex gap-1.5">
        <select
          value={selected?.id ?? ""}
          onChange={(e) => setCfgId(e.target.value)}
          aria-label="Vault config"
          className="min-w-0 flex-1 rounded-lg bg-black/20 px-2 py-1.5 text-xs text-foreground ring-1 ring-white/10 outline-none light:bg-black/5 light:ring-black/10"
        >
          {configs.map((c) => (<option key={c.id} value={c.id}>{c.name}</option>))}
        </select>
        <input
          value={pass}
          onChange={(e) => setPass(e.target.value)}
          type="password"
          placeholder="Password"
          aria-label="Vault password"
          className="w-24 shrink-0 rounded-lg bg-black/20 px-2 py-1.5 text-xs text-foreground ring-1 ring-white/10 outline-none placeholder:text-muted-foreground/50 light:bg-black/5 light:ring-black/10"
        />
        <input
          value={days}
          onChange={(e) => setDays(e.target.value.replace(/\D/g, ""))}
          placeholder="days"
          aria-label="Valid days (0 = forever)"
          dir="ltr"
          className="w-14 shrink-0 rounded-lg bg-black/20 px-2 py-1.5 text-center font-mono text-xs text-foreground ring-1 ring-white/10 outline-none light:bg-black/5 light:ring-black/10"
        />
      </div>
      <div className="flex gap-2">
        <button type="button" onClick={() => void exportVault()} disabled={!selected || !pass}
          className="flex-1 rounded-lg bg-surface-3 px-2 py-1.5 text-xs font-bold text-foreground transition-colors hover:bg-surface-4 disabled:opacity-50">
          Export .vault
        </button>
        <label className="flex-1 cursor-pointer rounded-lg bg-surface-3 px-2 py-1.5 text-center text-xs font-bold text-foreground transition-colors hover:bg-surface-4">
          Open .vault
          <input type="file" className="hidden" accept=".vault,.txt"
            onChange={(e) => { const f = e.target.files?.[0]; if (f) void importVault(f); e.target.value = ""; }} />
        </label>
      </div>
      {msg && <p className="px-1 text-[11px] text-muted-foreground">{msg}</p>}
    </div>
  );
}
