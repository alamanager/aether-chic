import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Check, Copy, Globe, MonitorUp, RefreshCw, Server } from "lucide-react";
import { Switch } from "@/components/ui/switch";
import { useConnectionStore } from "@/state/connectionStore";

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

function Row({
  icon,
  label,
  value,
  onCopy,
  copied,
}: {
  icon: React.ReactNode;
  label: string;
  value: string;
  onCopy?: () => void;
  copied?: boolean;
}) {
  return (
    <div className="flex items-center gap-2.5 rounded-xl bg-black/20 px-3 py-2 ring-1 ring-white/10 light:bg-black/5 light:ring-black/10">
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
 * Shown while Connected: local SOCKS endpoint, the separate HTTP endpoint
 * (bridge port = SOCKS port + 1, for consumers that only speak HTTP proxy),
 * the visible public IP, and the System Proxy switch (points Windows at the
 * HTTP endpoint, v2rayN-style). Errors surface as hints, never silently.
 * ponytail: public-IP lookup is a plain api.ipify.org fetch — swap for a
 * backend SOCKS-aware check if it ever proves unreliable.
 */
export function ConnectionInfo() {
  const status = useConnectionStore((s) => s.status);
  const [publicIp, setPublicIp] = useState<string | null>(null);
  const [ipLoading, setIpLoading] = useState(false);
  const [httpAddr, setHttpAddr] = useState<string | null>(null);
  const [sysProxy, setSysProxy] = useState<boolean | null>(null);
  const [proxyBusy, setProxyBusy] = useState(false);
  const [proxyHint, setProxyHint] = useState<string | null>(null);
  const [copied, setCopied] = useState<string | null>(null);

  const connected = status.state === "Connected";
  const socksAddr = connected ? status.socks_addr : null;
  const attemptId = useConnectionStore((s) => s.attemptId);
  // Which attempt the auto-apply below already ran for (a ref: re-running
  // on every render would flip the switch in a loop).
  const autoFor = useRef(0);

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

  useEffect(() => {
    // This component only mounts on the Idle→Connected transition, so this
    // is a genuine external-system sync (public-IP lookup + backend proxy
    // state), not a derivable render — same pattern as attemptStartedAt.
    /* eslint-disable react-hooks/set-state-in-effect */
    if (connected) {
      void fetchIp();
      invoke<string | null>("get_http_proxy")
        .then((a) => setHttpAddr(a))
        .catch(() => setHttpAddr(null));
      invoke<boolean>("get_system_proxy")
        .then(setSysProxy)
        .catch(() => setSysProxy(null));
    } else {
      setPublicIp(null);
      setHttpAddr(null);
      setSysProxy(null);
      setProxyHint(null);
      // Don't leave Windows pointing at a dead port after disconnect.
      if (sysProxy) {
        invoke("set_system_proxy", { enabled: false, server: "" }).catch(() => {});
      }
    }
    // Persisted choice: re-apply automatically on every connect so the
    // switch "just stays". Runs once per attempt, fire-and-forget — the UI
    // never blocks on it.
    if (connected && httpAddr && readSysProxyPref() && autoFor.current !== attemptId) {
      autoFor.current = attemptId;
      setProxyBusy(true);
      invoke("set_system_proxy", { enabled: true, server: httpAddr })
        .then(() => setSysProxy(true))
        .catch(() =>
          setProxyHint("Couldn't re-apply system proxy automatically — flip the switch manually."),
        )
        .finally(() => setProxyBusy(false));
    }
  }, [connected, httpAddr, attemptId, sysProxy, fetchIp]);
  /* eslint-enable react-hooks/set-state-in-effect */

  if (!connected || !socksAddr) return null;

  const doCopy = async (which: string, text: string) => {
    if (await copyText(text)) {
      setCopied(which);
      setTimeout(() => setCopied(null), 1500);
    }
  };

  const toggleSysProxy = async (on: boolean) => {
    if (on && !httpAddr) {
      setProxyHint("HTTP bridge isn't running — check the logs in Advanced.");
      return;
    }
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
        `Auto-switch unavailable (${String(e).slice(0, 90)}). Set it manually: Windows Settings → Proxy → ${httpAddr ?? socksAddr}`,
      );
    } finally {
      setProxyBusy(false);
    }
  };

  return (
    <div className="glass flex w-full max-w-sm flex-col gap-2 rounded-2xl p-3">
      <Row
        icon={<Server className="size-4" />}
        label="SOCKS5 proxy"
        value={socksAddr}
        onCopy={() => void doCopy("socks", socksAddr)}
        copied={copied === "socks"}
      />
      <Row
        icon={<Server className="size-4" />}
        label="HTTP proxy"
        value={httpAddr ?? "starting…"}
        onCopy={httpAddr ? () => void doCopy("http", httpAddr) : undefined}
        copied={copied === "http"}
      />
      <div className="flex items-center gap-2">
        <div className="min-w-0 flex-1">
          <Row
            icon={<Globe className="size-4" />}
            label="Visible IP"
            value={ipLoading ? "checking…" : (publicIp ?? "unavailable")}
            onCopy={publicIp ? () => void doCopy("ip", publicIp) : undefined}
            copied={copied === "ip"}
          />
        </div>
        <button
          type="button"
          onClick={() => void fetchIp()}
          aria-label="Refresh public IP"
          className="grid size-11 shrink-0 place-items-center rounded-xl text-muted-foreground ring-1 ring-white/10 transition-colors hover:bg-surface-3 hover:text-foreground"
        >
          <RefreshCw className={`size-4 ${ipLoading ? "animate-spin" : ""}`} />
        </button>
      </div>
      <div className="flex items-center justify-between rounded-xl px-1 py-1">
        <span className="flex items-center gap-2 text-xs text-muted-foreground">
          <MonitorUp className="size-4" />
          System proxy
          <span className="hidden font-mono text-[10px] opacity-70 sm:inline" dir="ltr">
            {httpAddr ?? socksAddr}
          </span>
        </span>
        <Switch
          checked={sysProxy ?? false}
          disabled={proxyBusy || sysProxy === null}
          onCheckedChange={(on) => void toggleSysProxy(on)}
          aria-label="Route system traffic through the tunnel (Windows system proxy)"
        />
      </div>
      {proxyHint && <p className="px-1 text-[11px] leading-5 text-muted-foreground">{proxyHint}</p>}
      {!proxyHint && sysProxy === null && (
        <p className="px-1 text-[11px] leading-5 text-muted-foreground">
          Flip System proxy to route Windows (HTTP) through the tunnel, or copy an address into
          apps that take SOCKS5 / HTTP manually.
        </p>
      )}
    </div>
  );
}
