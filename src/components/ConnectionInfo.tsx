import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AnimatePresence, motion } from "motion/react";
import { Check, Copy, Globe, MonitorUp, RefreshCw, Server } from "lucide-react";
import { Switch } from "@/components/ui/switch";
import { ClientsCard } from "@/components/ClientsCard";
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
    <div className="flex min-h-[50px] items-center gap-2.5 rounded-xl bg-black/25 px-3 py-2 ring-1 ring-white/10 light:bg-black/[0.04] light:ring-black/10">
      <span className="text-[#a5b4fc]">{icon}</span>
      <span className="min-w-0 flex-1 text-left">
        <span className="block text-[10px] font-semibold tracking-widest text-muted-foreground uppercase">
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
 * Live-session footer: rendered ONLY while an attempt is alive or connected
 * (status UI earns its pixels). Same endpoints/IP/sysproxy logic as before.
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
  const autoFor = useRef(0);

  const live =
    status.state === "Launching" ||
    status.state === "Connecting" ||
    status.state === "Reconnecting" ||
    status.state === "Connected";
  const connected = status.state === "Connected";
  const extra = useConnectionStore((s) => s.profile.extra_transport);
  const httpPortField = useConnectionStore((s) => s.profile.http_port);
  const torBind = useConnectionStore((s) => s.profile.tor_bind);
  const psiBind = useConnectionStore((s) => s.profile.psiphon_bind);
  const fromBind = addrsFromBind(bind);
  const isTorChain = extra === "tor" || extra === "tor_reverse";
  const isPsiChain = extra === "psiphon" || extra === "psiphon_reverse";
  // Effective exit listeners: explicit field or core default, loopback-
  // mapped for display — the same rule as the backend (profiles.rs).
  const normExit = (raw: string, def: string): string | null => {
    const v = (raw || "").trim() || def;
    const m = /^(.*):(\d+)\s*$/.exec(v);
    if (!m) return def;
    let h = m[1].replace(/^\[|\]$/g, "");
    if (h === "" || h === "0.0.0.0" || h === "::") h = "127.0.0.1";
    return `${h}:${m[2]}`;
  };
  const torExit = isTorChain ? normExit(torBind, "127.0.0.1:1820") : null;
  const psiExit = isPsiChain ? normExit(psiBind, "127.0.0.1:1821") : null;
  const socksAddr =
    status.state === "Connected"
      ? status.socks_addr
      : (torExit ?? psiExit ?? fromBind.socks);
  const portOf = (a: string): number | null => {
    const m = /:(\d+)\s*$/.exec(a);
    return m ? Number(m[1]) : null;
  };
  const hostOf = (a: string): string => {
    const m = /^(.*):\d+\s*$/.exec(a.trim());
    return m ? m[1] : "127.0.0.1";
  };
  const socksPort = portOf(socksAddr) ?? portOf(fromBind.socks) ?? 1819;
  // Same collision rule as the backend: custom wins unless it equals the
  // SOCKS or an exit listener port; otherwise SOCKS+1 (1822 for Tor
  // chains), bumped past any taken port.
  const taken = new Set<number>([socksPort]);
  const tp = torExit ? portOf(torExit) : null;
  if (tp !== null) taken.add(tp);
  const pp = psiExit ? portOf(psiExit) : null;
  if (pp !== null) taken.add(pp);
  const customPort = (() => {
    const n = Number(httpPortField);
    return httpPortField.trim() !== "" &&
      Number.isInteger(n) &&
      n >= 1 &&
      n <= 65535 &&
      !taken.has(n)
      ? n
      : null;
  })();
  let httpPort = customPort ?? (isTorChain ? 1822 : socksPort >= 65535 ? 1820 : socksPort + 1);
  for (let i = 0; i < 4 && taken.has(httpPort); i++) {
    httpPort = httpPort >= 65535 ? 1820 : httpPort + 1;
  }
  const httpAddr = `${hostOf(fromBind.socks)}:${httpPort}`;

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

  useEffect(() => {
    invoke<boolean>("get_system_proxy")
      .then(setSysProxy)
      .catch(() => setSysProxy(null));
  }, []);
  /* eslint-disable react-hooks/set-state-in-effect */
  useEffect(() => {
    if (!live) return;
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
  }, [live, connected, attemptId, sysProxy, fetchIp, toggleSysProxy]);
  /* eslint-enable react-hooks/set-state-in-effect */

  const doCopy = async (which: string, text: string) => {
    if (await copyText(text)) {
      setCopied(which);
      setTimeout(() => setCopied(null), 1500);
    }
  };

  return (
    <AnimatePresence initial={false}>
      {live && (
        <motion.div
          key="session-footer"
          initial={{ opacity: 0, y: 12 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: 8 }}
          transition={{ duration: 0.22, ease: [0.22, 1, 0.36, 1] }}
          className="glass flex w-full max-w-sm flex-col gap-2 rounded-2xl p-3"
        >
          <div className="flex h-6 items-center justify-between px-1">
            <span className="text-[10px] font-bold tracking-[0.2em] text-muted-foreground uppercase">
              Live session
            </span>
            <span
              className={cn(
                "flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-[10px] font-bold tracking-wider",
                connected ? "bg-status-connected/15 text-status-connected" : "bg-surface-3 text-muted-foreground",
              )}
            >
              <span
                className={cn(
                  "size-1.5 rounded-full",
                  connected ? "bg-status-connected" : "anim-glow-fast bg-status-connecting",
                )}
                aria-hidden
              />
              {connected ? "LIVE" : "WORKING"}
            </span>
          </div>
          <Row icon={<Server className="size-4" />} label="SOCKS5" value={socksAddr}
            onCopy={() => void doCopy("socks", socksAddr)} copied={copied === "socks"} />
          <Row icon={<Server className="size-4" />} label="HTTP" value={httpAddr}
            onCopy={() => void doCopy("http", httpAddr)} copied={copied === "http"} />
          {torExit && (
            <Row icon={<Server className="size-4" />} label="Tor exit" value={torExit}
              onCopy={() => void doCopy("tor", torExit)} copied={copied === "tor"} />
          )}
          {psiExit && (
            <Row icon={<Server className="size-4" />} label="Psiphon exit" value={psiExit}
              onCopy={() => void doCopy("psi", psiExit)} copied={copied === "psi"} />
          )}
          <div className="flex items-center gap-2">
            <div className="min-w-0 flex-1">
              <Row icon={<Globe className="size-4" />} label="Exit IP"
                value={ipLoading ? "measuring…" : (publicIp ?? "—")}
                onCopy={publicIp ? () => void doCopy("ip", publicIp) : undefined}
                copied={copied === "ip"} />
            </div>
            <button
              type="button"
              onClick={() => void fetchIp()}
              aria-label="Re-measure exit IP"
              className="grid size-11 shrink-0 place-items-center self-stretch rounded-xl text-muted-foreground ring-1 ring-white/10 transition-colors hover:bg-surface-3 hover:text-foreground"
            >
              <RefreshCw className={`size-4 ${ipLoading ? "animate-spin" : ""}`} />
            </button>
          </div>
          <div className="flex min-h-[44px] items-center justify-between rounded-xl px-1">
            <span className="flex items-center gap-2 text-xs text-muted-foreground">
              <MonitorUp className="size-4" />
              System proxy
            </span>
            <Switch
              checked={sysProxy ?? false}
              disabled={proxyBusy || sysProxy === null}
              onCheckedChange={(on) => void toggleSysProxy(on)}
              aria-label="Route system traffic through the tunnel (Windows system proxy)"
            />
          </div>
          {proxyHint && <p className="px-1 text-[11px] leading-5 text-muted-foreground">{proxyHint}</p>}
          <ClientsCard />
        </motion.div>
      )}
    </AnimatePresence>
  );
}
