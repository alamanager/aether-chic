import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Check, Copy, Globe, MonitorUp, RefreshCw, Server } from "lucide-react";
import { Switch } from "@/components/ui/switch";
import { useConnectionStore } from "@/state/connectionStore";

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
 * Shown while Connected: local SOCKS endpoint, the visible public IP
 * (fetched through the tunnel when System Proxy is on), and the
 * System Proxy switch. The switch calls the backend `set_system_proxy`
 * command; if the backend predates that command the error is surfaced
 * as a hint instead of failing silently.
 * ponytail: public-IP lookup is a plain api.ipify.org fetch — swap for a
 * backend SOCKS-aware check if it ever proves unreliable.
 */
export function ConnectionInfo() {
  const status = useConnectionStore((s) => s.status);
  const [publicIp, setPublicIp] = useState<string | null>(null);
  const [ipLoading, setIpLoading] = useState(false);
  const [sysProxy, setSysProxy] = useState<boolean | null>(null);
  const [proxyBusy, setProxyBusy] = useState(false);
  const [proxyHint, setProxyHint] = useState<string | null>(null);
  const [copied, setCopied] = useState<string | null>(null);

  const connected = status.state === "Connected";
  const socksAddr = connected ? status.socks_addr : null;

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
      invoke<boolean>("get_system_proxy")
        .then(setSysProxy)
        .catch(() => setSysProxy(null));
    } else {
      setPublicIp(null);
      setSysProxy(null);
      setProxyHint(null);
    }
  }, [connected, fetchIp]);
  /* eslint-enable react-hooks/set-state-in-effect */

  if (!connected || !socksAddr) return null;

  const doCopy = async (which: string, text: string) => {
    if (await copyText(text)) {
      setCopied(which);
      setTimeout(() => setCopied(null), 1500);
    }
  };

  const toggleSysProxy = async (on: boolean) => {
    setProxyBusy(true);
    setProxyHint(null);
    try {
      await invoke("set_system_proxy", { enabled: on, server: socksAddr });
      setSysProxy(on);
    } catch (e) {
      // Backend without the sysproxy module (or a non-Windows target):
      // leave the switch off and explain the manual path.
      setSysProxy(false);
      setProxyHint(
        `Auto-switch unavailable (${String(e).slice(0, 90)}). Set it manually: Windows Settings → Proxy → ${socksAddr}`,
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
            {socksAddr}
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
          Copy the SOCKS5 address into your app, or flip System proxy to route Windows through it.
        </p>
      )}
    </div>
  );
}
