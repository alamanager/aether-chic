import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Check, Copy, Globe, MonitorUp, RefreshCw, Server } from "lucide-react";
import { Switch } from "@/components/ui/switch";
import { useConnectionStore } from "@/state/connectionStore";
import { cn } from "@/lib/utils";

const SYS_PROXY_PREF = "aether-sysproxy";

function readSysProxyPref(): boolean {
  try {
    return localStorage.getItem(SYS_PROXY_PREF) === "1";
  } catch {
    return false;
  }
}

function writeSysProxyPref(on: boolean) {
  try {
    if (on) localStorage.setItem(SYS_PROXY_PREF, "1");
    else localStorage.removeItem(SYS_PROXY_PREF);
  } catch {
    /* private mode — preference just won't persist */
  }
}

async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    try {
      const ta = document.createElement("textarea");
      ta.value = text;
      document.body.appendChild(ta);
      ta.select();
      document.execCommand("copy");
      ta.remove();
      return true;
    } catch {
      return false;
    }
  }
}

/** Local endpoints, derived synchronously from the profile — no waiting on
 * the backend, so the card renders identically before and after connect.
 * Unspecified binds map to loopback, mirroring the backend. */
function addrsFromBind(bind: string): { socks: string; http: string } {
  let host = "127.0.0.1";
  let port = 1819;
  const m = /^(.*):(\d+)\s*$/.exec(bind.trim());
  if (m) {
    const h = m[1].replace(/^\[|\]$/g, "");
    if (h !== "" && h !== "0.0.0.0" && h !== "::") host = h;
    const p = Number(m[2]);
    if (p >= 1 && p <= 65535) port = p;
  }
  const httpPort = port >= 65535 ? 1 : port + 1;
  return { socks: `${host}:${port}`, http: `${host}:${httpPort}` };
}

/** HTTP twin of a live "host:port" endpoint (port + 1). */
function httpPortOf(addr: string): string {
  const m = /^(.*):(\d+)\s*$/.exec(addr.trim());
  if (!m) return addr;
  const p = Number(m[2]);
  return `${m[1]}:${p >= 65535 ? 1 : p + 1}`;
}

function Row({
  icon,
  label,
  value,
  onCopy,
  copied,
  dimmed,
}: {
  icon: React.ReactNode;
  label: string;
  value: string;
  onCopy?: () => void;
  copied?: boolean;
  dimmed?: boolean;
}) {
  return (
    <div
      className={cn(
        "flex min-h-[52px] items-center gap-2.5 rounded-xl px-3 py-2 ring-1 transition-opacity",
        dimmed
          ? "bg-black/10 opacity-60 ring-white/5 light:bg-black/[0.03] light:ring-black/5"
          : "bg-black/20 ring-white/10 light:bg-black/5 light:ring-black/10",
      )}
    >
      <span className="text-muted-foreground">{icon}</span>
      <span className="min-w-0 flex-1 text-left">
        <span className="block text-[10px] font-medium tracking-wide text-muted-foreground uppercase">
          {label}
        </span>
        <span className="block truncate font-mono text-xs text-foreground" dir="ltr">
          {value}
        </span>
      </span>
      {onCopy && (
        <button
          type="button"
          onClick={onCopy}
          aria-label={`Copy ${label}`}
          className="grid size-8 shrink-0 place-items-center rounded-lg text-muted-foreground transition-colors hover:bg-surface-3 hover:text-foreground"
        >
          {copied ? <Check className="size-3.5 text-status-connected" /> : <Copy className="size-3.5" />}
        </button>
      )}
    </div>
  );
}

/**
 * Fixed-size proxy card, always mounted: same box in every state, so
 * connecting never shifts the layout. Addresses come straight from the
 * profile (no async wait), the switch works before connect, and the LIVE
 * chip flips on with the tunnel.
 * ponytail: public-IP lookup is a plain api.ipify.org fetch — swap for a
 * backend SOCKS-aware check if it ever proves unreliable.
 */
export function ConnectionInfo() {
  const status = useConnectionStore((s) => s.status);
  const bind = useConnectionStore((s) => s.profile.bind_address);
  const attemptId = useConnectionStore((s) => s.attemptId);
  const [publicIp, setPublicIp] = useState<string | null>(null);
  const [ipLoading, setIpLoading] = useState(false);
  const [sysProxy, setSysProxy] = useState<boolean | null>(null);
  const [proxyBusy, setProxyBusy] = useState(false);
  const [proxyHint, setProxyHint] = useState<string | null>(null);
  const [copied, setCopied] = useState<string | null>(null);
  // Which attempt the auto-apply below already ran for (a ref: re-running
  // on every render would flip the switch in a loop).
  const autoFor = useRef(0);

  const connected = status.state === "Connected";
  const extra = useConnectionStore((s) => s.profile.extra_transport);
  const torBind = useConnectionStore((s) => s.profile.tor_bind);
  const psiBind = useConnectionStore((s) => s.profile.psiphon_bind);
  const fromBind = addrsFromBind(bind);
  // While connected, trust the backend-reported endpoints (direct mode
  // serves 11819/11820, not the profile ports); otherwise show what a
  // connect will use. Inline state checks so TS narrows the union.
  const socksAddr =
    status.state === "Connected" ? status.socks_addr : fromBind.socks;
  // Tor/Psiphon exit listeners for inside/reverse modes (mirrors the
  // backend resolution incl. defaults); the HTTP row moves aside on clash.
  const normExit = (raw: string, def: string): string | null => {
    const v = (raw || "").trim() || def;
    const m = /^(.*):(\d+)\s*$/.exec(v);
    if (!m) return def;
    let h = m[1].replace(/^\[|\]$/g, "");
    if (h === "" || h === "0.0.0.0" || h === "::") h = "127.0.0.1";
    return `${h}:${m[2]}`;
  };
  const torExit =
    extra === "tor" || extra === "tor_reverse" ? normExit(torBind, "127.0.0.1:1820") : null;
  const psiExit =
    extra === "psiphon" || extra === "psiphon_reverse"
      ? normExit(psiBind, "127.0.0.1:1821")
      : null;
  const portOf = (a: string): number | null => {
    const m = /:(\d+)\s*$/.exec(a);
    return m ? Number(m[1]) : null;
  };
  let httpAddr =
    status.state === "Connected" ? httpPortOf(status.socks_addr) : fromBind.http;
  {
    // Same collision rule as the backend: never show an HTTP address that
    // equals the SOCKS or an exit listener port.
    const taken = new Set<number>();
    const sp = portOf(socksAddr);
    if (sp !== null) taken.add(sp);
    const tp = torExit ? portOf(torExit) : null;
    if (tp !== null) taken.add(tp);
    const pp = psiExit ? portOf(psiExit) : null;
    if (pp !== null) taken.add(pp);
    let hp = portOf(httpAddr);
    for (let i = 0; i < 3 && hp !== null && taken.has(hp); i++) {
      hp = hp >= 65535 ? 1820 : hp + 1;
      const m = /^(.*):\d+\s*$/.exec(httpAddr);
      if (m) httpAddr = `${m[1]}:${hp}`;
    }
  }

  const fetchIp = useCallback(async () => {
    setIpLoading(true);
    try {
      const ctrl = new AbortController();
      const t = setTimeout(() => ctrl.abort(), 8000);
      const res = await fetch("https://api.ipify.org?format=json", { signal: ctrl.signal });
      clearTimeout(t);
      const data = (await res.json()) as { ip?: string };
      setPublicIp(data.ip ?? null);
    } catch {
      setPublicIp(null);
    } finally {
      setIpLoading(false);
    }
  }, []);

  const toggleSysProxy = useCallback(
    async (on: boolean) => {
      if (on && !httpAddr) return;
      setProxyBusy(true);
      setProxyHint(null);
      try {
        await invoke("set_system_proxy", { enabled: on, server: on ? httpAddr : socksAddr });
        setSysProxy(on);
        writeSysProxyPref(on);
      } catch (e) {
        // Backend without the sysproxy module (or a non-Windows target):
        // leave the switch off and explain the manual path.
        setSysProxy(false);
        setProxyHint(
          `Auto-switch unavailable (${String(e).slice(0, 90)}). Set it manually: Windows Settings → Proxy → ${httpAddr}`,
        );
      } finally {
        setProxyBusy(false);
      }
    },
    [httpAddr, socksAddr],
  );

  // Ground truth once (backend may predate the command), then per attempt:
  // refresh IP, auto-apply the persisted choice, and never leave Windows
  // pointing at a dead port after disconnect.
  useEffect(() => {
    invoke<boolean>("get_system_proxy")
      .then(setSysProxy)
      .catch(() => setSysProxy(null));
  }, []);
  /* eslint-disable react-hooks/set-state-in-effect */
  useEffect(() => {
    if (connected) {
      void fetchIp();
      if (readSysProxyPref() && autoFor.current !== attemptId) {
        autoFor.current = attemptId;
        void toggleSysProxy(true);
      }
    } else {
      setPublicIp(null);
      setProxyHint(null);
      if (sysProxy) {
        invoke("set_system_proxy", { enabled: false, server: "" }).catch(() => {});
      }
    }
  }, [connected, attemptId, sysProxy, fetchIp, toggleSysProxy]);
  /* eslint-enable react-hooks/set-state-in-effect */

  const doCopy = async (which: string, text: string) => {
    if (await copyText(text)) {
      setCopied(which);
      setTimeout(() => setCopied(null), 1500);
    }
  };

  return (
    <div className="glass flex w-full max-w-sm flex-col gap-2 rounded-2xl p-3">
      <div className="flex h-6 items-center justify-between px-1">
        <span className="text-[10px] font-semibold tracking-widest text-muted-foreground uppercase">
          Local proxy
        </span>
        <span
          className={cn(
            "flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-[10px] font-bold tracking-wider",
            connected
              ? "bg-status-connected/15 text-status-connected"
              : "bg-surface-3 text-muted-foreground",
          )}
        >
          <span
            className={cn(
              "size-1.5 rounded-full",
              connected ? "bg-status-connected" : "bg-status-idle",
            )}
            aria-hidden
          />
          {connected ? "LIVE" : "OFFLINE"}
        </span>
      </div>
      <Row
        icon={<Server className="size-4" />}
        label="SOCKS5 proxy"
        value={socksAddr}
        onCopy={() => void doCopy("socks", socksAddr)}
        copied={copied === "socks"}
        dimmed={!connected}
      />
      <Row
        icon={<Server className="size-4" />}
        label="HTTP proxy"
        value={httpAddr}
        onCopy={() => void doCopy("http", httpAddr)}
        copied={copied === "http"}
        dimmed={!connected}
      />
      {torExit && (
        <Row
          icon={<Server className="size-4" />}
          label="Tor exit"
          value={torExit}
          onCopy={() => void doCopy("tor", torExit)}
          copied={copied === "tor"}
          dimmed={!connected}
        />
      )}
      {psiExit && (
        <Row
          icon={<Server className="size-4" />}
          label="Psiphon exit"
          value={psiExit}
          onCopy={() => void doCopy("psi", psiExit)}
          copied={copied === "psi"}
          dimmed={!connected}
        />
      )}
      <div className="flex items-center gap-2">
        <div className="min-w-0 flex-1">
          <Row
            icon={<Globe className="size-4" />}
            label="Visible IP"
            value={ipLoading ? "checking…" : (publicIp ?? "—")}
            onCopy={publicIp ? () => void doCopy("ip", publicIp) : undefined}
            copied={copied === "ip"}
            dimmed={!connected}
          />
        </div>
        <button
          type="button"
          onClick={() => void fetchIp()}
          aria-label="Refresh public IP"
          className="grid size-11 shrink-0 place-items-center self-stretch rounded-xl text-muted-foreground ring-1 ring-white/10 transition-colors hover:bg-surface-3 hover:text-foreground"
        >
          <RefreshCw className={`size-4 ${ipLoading ? "animate-spin" : ""}`} />
        </button>
      </div>
      <div className="flex min-h-[44px] items-center justify-between rounded-xl px-1">
        <span className="flex items-center gap-2 text-xs text-muted-foreground">
          <MonitorUp className="size-4" />
          System proxy
          <span className="hidden font-mono text-[10px] opacity-70 sm:inline" dir="ltr">
            {httpAddr}
          </span>
        </span>
        <Switch
          checked={sysProxy ?? false}
          disabled={proxyBusy || sysProxy === null}
          onCheckedChange={(on) => void toggleSysProxy(on)}
          aria-label="Route system traffic through the tunnel (Windows system proxy)"
        />
      </div>
      <p className="min-h-5 px-1 text-[11px] leading-5 text-muted-foreground">
        {proxyHint ??
          (sysProxy === null
            ? "System proxy control needs the latest backend."
            : "Flip System proxy any time — even before connecting.")}
      </p>
    </div>
  );
}
