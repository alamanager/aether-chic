import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  ConnectionProfile,
  ConnectionStatus,
  ExtraTransport,
  LogLine,
  Noize,
  ZeroTrustAuth,
} from "@/types/connection";

const MAX_LOG_LINES = 500;

interface ConnectionState {
  status: ConnectionStatus;
  profile: ConnectionProfile;
  logs: LogLine[];
  sidecarError: string | null;
  /** Aether's own route-probe budget in seconds, parsed live out of its log
   * stream (its prober logs e.g. "...budget=120s" once scanning starts) —
   * lets the UI show real progress instead of an indefinite spinner. Reset
   * on every fresh attempt since it can differ by protocol/scan mode. */
  scanBudgetSecs: number | null;
  /** Last Tor bootstrap % seen in the log stream (null when none). Updated
   * in flushLogs so status text can subscribe to one number instead of the
   * whole log array (which re-renders every 100ms during scans). */
  torPercent: number | null;
  /** Live transfer counters + per-second rates (null until the first
   * stats line arrives). Reset on every fresh attempt. */
  traffic: { up: number; down: number; upRate: number; downRate: number; at: number } | null;
  /** Ring buffer of per-report rates for the graph (~2s each, max 120 ≈
   * 4 min). Nulls mark disconnect gaps so the line breaks visibly. */
  samples: { t: number; down: number | null; up: number | null }[];
  /** Monotonic key for controls that must reset between explicit connects. */
  attemptId: number;
  connect: () => Promise<void>;
  disconnect: () => Promise<void>;
  setProtocol: (protocol: ConnectionProfile["protocol"]) => void;
  setScanMode: (scan_mode: ConnectionProfile["scan_mode"]) => void;
  setIpVersion: (ip_version: ConnectionProfile["ip_version"]) => void;
  setQuickReconnect: (quick_reconnect: boolean) => void;
  setMasqueHttp2: (masque_http2: boolean) => void;
  setNoize: (noize: Noize) => void;
  setBindAddress: (bind_address: string) => void;
  setHttpPort: (http_port: string) => void;
  setDns: (dns: string) => void;
  setZeroTrustTeam: (zero_trust_team: string) => void;
  setZeroTrustAuth: (zero_trust_auth: ZeroTrustAuth) => void;
  setAccessEmail: (access_email: string) => void;
  setAccessClientId: (access_client_id: string) => void;
  setAccessClientSecret: (access_client_secret: string) => void;
  setAccessToken: (access_token: string) => void;
  setZeroTrustGateway: (zero_trust_gateway: boolean) => void;
  setRouteBlock: (route_block: string) => void;
  setRouteDirect: (route_direct: string) => void;
  setRoutesFile: (routes_file: string) => void;
  setUpstream: (upstream: string) => void;
  setExtraTransport: (extra_transport: ExtraTransport) => void;
  setPsiphonRegion: (psiphon_region: string) => void;
  setTorBridges: (tor_bridges: boolean) => void;
  setPsiphonMode: (psiphon_mode: ConnectionProfile["psiphon_mode"]) => void;
  setTorRelays: (tor_relays: string) => void;
  setTorBridge: (tor_bridge: string) => void;
  setTorBridgeFile: (tor_bridge_file: string) => void;
  setTorBind: (tor_bind: string) => void;
  setPsiphonConfig: (psiphon_config: string) => void;
  setPsiphonCdnIps: (psiphon_cdn_ips: string) => void;
  setPsiphonCdnSni: (psiphon_cdn_sni: string) => void;
  setPsiphonCdnSets: (psiphon_cdn_sets: string) => void;
  setPsiphonServerEntries: (psiphon_server_entries: string) => void;
  setPsiphonBind: (psiphon_bind: string) => void;
  setPeer: (peer: string) => void;
  setWgPeer: (wg_peer: string) => void;
  setGoolPeer: (gool_peer: string) => void;
  setWiwOuter: (wiw_outer: string) => void;
  setWiwInner: (wiw_inner: string) => void;
  setMimOuter: (mim_outer: string) => void;
  setMimInner: (mim_inner: string) => void;
  setExitLoc: (exit_loc: string) => void;
  setApiFragment: (api_fragment: boolean) => void;
  setNoQuicV2: (no_quic_v2: boolean) => void;
  setH2Peer: (h2_peer: string) => void;
  setNoDataCheck: (no_data_check: boolean) => void;
  setNoProfileRetry: (no_profile_retry: boolean) => void;
  setKeepalive: (keepalive: string) => void;
  setPerf: (perf: string) => void;
  setEch: (ech: string) => void;
  setEchDns: (ech_dns: string) => void;
  setEchDomain: (ech_domain: string) => void;
  setTlsCiphers: (tls_ciphers: string) => void;
  setTlsGroups: (tls_groups: string) => void;
  setDisableGrease: (disable_grease: boolean) => void;
  setMasqueMtu: (masque_mtu: string) => void;
  setNetstackRx: (netstack_rx: string) => void;
  setNetstackTx: (netstack_tx: string) => void;
  setRouteSniff: (route_sniff: boolean) => void;
  clearLogs: () => void;
  retryAfterSidecarError: () => void;
  /** Manual (sidecar) mode: user configs instead of the WARP scan. */
  manualMode: boolean;
  setManualMode: (on: boolean) => void;
  configs: ManualConfig[];
  activeConfigId: string | null;
  addConfigs: (items: ManualConfig[]) => void;
  removeConfig: (id: string) => void;
  setActiveConfig: (id: string | null) => void;
  setConfigLatency: (id: string, ms: number | null) => void;
  setConfigVia: (id: string, via: string | null) => void;
}

export interface ManualConfig {
  id: string;
  name: string;
  engine: "singbox" | "xray" | "ssh" | "ikev2";
  /** Engine outbound object (sing-box `type` / xray `protocol` shape). */
  outbound?: unknown;
  /** Pasted full runtime config (used verbatim). */
  rawJson?: unknown;
  socksPort: number;
  host: string;
  port: number;
  source: string;
  latencyMs?: number | null;
  /** SSH/IKEv2 credential material (key text or password). */
  username?: string;
  secret?: string;
  /** Chain through another config of the same engine (its id). */
  via?: string | null;
}

const MANUAL_KEY = "aether-manual-v1";

function loadManual(): { manualMode: boolean; configs: ManualConfig[]; activeConfigId: string | null } {
  try {
    const raw = localStorage.getItem(MANUAL_KEY);
    if (!raw) return { manualMode: false, configs: [], activeConfigId: null };
    const v = JSON.parse(raw) as Partial<{
      manualMode: boolean;
      configs: ManualConfig[];
      activeConfigId: string | null;
    }>;
    return {
      manualMode: v.manualMode === true,
      configs: Array.isArray(v.configs) ? v.configs : [],
      activeConfigId: v.activeConfigId ?? null,
    };
  } catch {
    return { manualMode: false, configs: [], activeConfigId: null };
  }
}

/** Runtime config wrapping one outbound (mirrors links::singbox_config). */
export function buildSingboxRuntime(outbound: unknown): string {
  const proxy = (
    typeof outbound === "object" && outbound !== null ? { ...(outbound as Record<string, unknown>) } : {}
  ) as Record<string, unknown>;
  proxy.tag = "proxy";
  return JSON.stringify({
    log: { level: "info" },
    inbounds: [
      { type: "mixed", tag: "in", listen: "127.0.0.1", listen_port: 1870, sniff: true },
    ],
    outbounds: [proxy, { type: "direct", tag: "direct" }],
    route: { final: "proxy", auto_detect_interface: true },
    experimental: { clash_api: { external_controller: "127.0.0.1:1872" } },
  });
}

/** Full runtime JSON for one manual config, honoring `via` chains
 * (same-engine only — mixed chains are refused loudly, never guessed). */
export function buildManualRuntime(all: ManualConfig[], cfg: ManualConfig): string {
  if (cfg.rawJson !== undefined && cfg.rawJson !== null) return JSON.stringify(cfg.rawJson);
  if (cfg.engine === "ssh") {
    return JSON.stringify({
      host: cfg.host,
      port: cfg.port,
      user: cfg.username ?? "",
      key: cfg.secret ?? "",
    });
  }
  const via = cfg.via ? all.find((c) => c.id === cfg.via) : undefined;
  if (via && via.engine !== cfg.engine) {
    throw new Error(`Chain engine mismatch (${cfg.engine} via ${via.engine})`);
  }
  const base = cfg.engine === "xray" ? JSON.parse(buildXrayRuntime(cfg.outbound)) : JSON.parse(buildSingboxRuntime(cfg.outbound));
  if (!via || via.outbound === undefined) return JSON.stringify(base);
  const hop = (
    typeof via.outbound === "object" && via.outbound !== null ? { ...(via.outbound as Record<string, unknown>) } : {}
  ) as Record<string, unknown>;
  if (cfg.engine === "singbox") {
    hop.tag = "via";
    const outs = (base.outbounds as Record<string, unknown>[]);
    const proxy = outs[0] as Record<string, unknown>;
    proxy.detour = "via";
    outs.splice(1, 0, hop);
  } else {
    hop.tag = "via";
    const outs = (base.outbounds as Record<string, unknown>[]);
    const proxy = outs[0] as Record<string, unknown>;
    proxy.proxySettings = { tag: "via", transportLayer: true };
    outs.splice(1, 0, hop);
  }
  return JSON.stringify(base);
}
export function buildXrayRuntime(outbound: unknown): string {
  const proxy = (
    typeof outbound === "object" && outbound !== null ? { ...(outbound as Record<string, unknown>) } : {}
  ) as Record<string, unknown>;
  proxy.tag = "proxy";
  return JSON.stringify({
    log: { loglevel: "warning" },
    inbounds: [
      { protocol: "socks", listen: "127.0.0.1", port: 1870, settings: { udp: true } },
      { protocol: "http", listen: "127.0.0.1", port: 1871 },
    ],
    outbounds: [proxy, { protocol: "freedom" }],
  });
}

export const useConnectionStore = create<ConnectionState>((set, get) => {
  const savedManual = loadManual();
  const persistManual = () => {
    try {
      const s = get();
      localStorage.setItem(
        MANUAL_KEY,
        JSON.stringify({ manualMode: s.manualMode, configs: s.configs, activeConfigId: s.activeConfigId }),
      );
    } catch {
      /* private mode */
    }
  };
  return {
  status: { state: "Idle" },
  manualMode: savedManual.manualMode,
  configs: savedManual.configs,
  activeConfigId: savedManual.activeConfigId,
  profile: {
    protocol: "auto",
    scan_mode: "balanced",
    ip_version: "v4",
    quick_reconnect: true,
    masque_http2: false,
    noize: "firewall",
    bind_address: "127.0.0.1:1819",
    http_port: "",
    dns: "",
    zero_trust_team: "",
    zero_trust_auth: "email",
    access_email: "",
    access_client_id: "",
    access_client_secret: "",
    access_token: "",
    zero_trust_gateway: false,
    route_block: "",
    route_direct: "",
    routes_file: "",
    upstream: "",
    extra_transport: "none",
    psiphon_region: "",
    tor_bridges: false,
    psiphon_mode: "auto",
    tor_relays: "",
    tor_bridge: "",
    tor_bridge_file: "",
    tor_bind: "",
    psiphon_config: "",
    psiphon_cdn_ips: "",
    psiphon_cdn_sni: "",
    psiphon_cdn_sets: "",
    psiphon_server_entries: "",
    psiphon_bind: "",
    peer: "",
    wg_peer: "",
    gool_peer: "",
    wiw_outer: "",
    wiw_inner: "",
    mim_outer: "",
    mim_inner: "",
    exit_loc: "",
    api_fragment: false,
    no_quic_v2: false,
    h2_peer: "",
    no_data_check: false,
    no_profile_retry: false,
    keepalive: "",
    perf: "",
    ech: "",
    ech_dns: "",
    ech_domain: "",
    tls_ciphers: "",
    tls_groups: "",
    disable_grease: false,
    masque_mtu: "",
    netstack_rx: "",
    netstack_tx: "",
    route_sniff: true,
  },
  logs: [],
  sidecarError: null,
  scanBudgetSecs: null,
  torPercent: null,
  traffic: null,
  samples: [],
  attemptId: 0,

  connect: async () => {
    // A fresh user-initiated attempt should not inherit stale log-driven UI
    // prompts (notably a previous Zero Trust email-code request).
    set((s) => ({ logs: [], scanBudgetSecs: null, torPercent: null, traffic: null, samples: [], attemptId: s.attemptId + 1 }));
    if (get().manualMode) {
      const s = get();
      const cfg = s.configs.find((c) => c.id === s.activeConfigId);
      if (!cfg) {
        set({ status: { state: "Error", message: "Pick a config in Tune → Manual first.", phase: "launching" } });
        return;
      }
      try {
        if (cfg.engine === "ikev2") {
          await invoke("manual_ikev2_up", {
            server: cfg.host,
            user: cfg.username ?? "",
            pass: cfg.secret ?? "",
            name: cfg.name,
          });
          return;
        }
        const configJson = buildManualRuntime(s.configs, cfg);
        await invoke("manual_connect", { engine: cfg.engine, configJson });
      } catch (e) {
        const message = String(e);
        if (message.toLowerCase().includes("binary not found")) {
          set({ sidecarError: message });
        } else {
          set({ status: { state: "Error", message, phase: "launching" } });
        }
      }
      return;
    }
    try {
      await invoke("connect", { profileOverride: get().profile });
    } catch (e) {
      const message = String(e);
      // "Binary not found" (src-tauri/src/aether/mod.rs::resolve_binary) means
      // the tunnel engine itself can't run at all — structurally different
      // from a normal connection failure, so it routes to the full-screen
      // SidecarErrorScreen instead of the button's own error state.
      if (message.toLowerCase().includes("binary not found")) {
        set({ sidecarError: message });
      } else {
        set({ status: { state: "Error", message, phase: "launching" } });
      }
    }
  },

    disconnect: async () => {
    try {
      const s = get();
      const cfg = s.manualMode ? s.configs.find((c) => c.id === s.activeConfigId) : undefined;
      if (cfg?.engine === "ikev2") {
        // The OS tunnel outlives our process kill — tear it down first.
        await invoke("manual_ikev2_down").catch(() => {});
      }
      await invoke("disconnect");
    } catch {
      // Backend rejects disconnect() when there's nothing to stop (already
      // Idle) — nothing for the UI to do since status already reflects that.
    }
  },

  setProtocol: (protocol) =>
    set((s) => ({
      profile: {
        ...s.profile,
        protocol,
      },
    })),

  setScanMode: (scan_mode) =>
    set((s) => ({ profile: { ...s.profile, scan_mode } })),

  setIpVersion: (ip_version) =>
    set((s) => ({ profile: { ...s.profile, ip_version } })),

  setQuickReconnect: (quick_reconnect) =>
    set((s) => ({ profile: { ...s.profile, quick_reconnect } })),

  setMasqueHttp2: (masque_http2) =>
    set((s) => ({ profile: { ...s.profile, masque_http2 } })),

  setNoize: (noize) =>
    set((s) => ({ profile: { ...s.profile, noize } })),

  setBindAddress: (bind_address) =>
    set((s) => ({ profile: { ...s.profile, bind_address } })),

  setHttpPort: (http_port) =>
    set((s) => ({ profile: { ...s.profile, http_port } })),

  setDns: (dns) => set((s) => ({ profile: { ...s.profile, dns } })),

  setZeroTrustTeam: (zero_trust_team) =>
    set((s) => ({ profile: { ...s.profile, zero_trust_team } })),

  setZeroTrustAuth: (zero_trust_auth) =>
    set((s) => ({
      profile: {
        ...s.profile,
        zero_trust_auth,
        access_email: "",
        access_client_id: "",
        access_client_secret: "",
        access_token: "",
      },
    })),

  setAccessEmail: (access_email) =>
    set((s) => ({ profile: { ...s.profile, access_email } })),

  setAccessClientId: (access_client_id) =>
    set((s) => ({ profile: { ...s.profile, access_client_id } })),

  setAccessClientSecret: (access_client_secret) =>
    set((s) => ({ profile: { ...s.profile, access_client_secret } })),

  setAccessToken: (access_token) =>
    set((s) => ({ profile: { ...s.profile, access_token } })),

  setZeroTrustGateway: (zero_trust_gateway) =>
    set((s) => ({ profile: { ...s.profile, zero_trust_gateway } })),

  setRouteBlock: (route_block) =>
    set((s) => ({ profile: { ...s.profile, route_block } })),

  setRouteDirect: (route_direct) =>
    set((s) => ({ profile: { ...s.profile, route_direct } })),

  setRoutesFile: (routes_file) =>
    set((s) => ({ profile: { ...s.profile, routes_file } })),

  setUpstream: (upstream) =>
    set((s) => ({ profile: { ...s.profile, upstream } })),

  setExtraTransport: (extra_transport) =>
    set((s) => ({ profile: { ...s.profile, extra_transport } })),

  setPsiphonRegion: (psiphon_region) =>
    set((s) => ({ profile: { ...s.profile, psiphon_region } })),

  setTorBridges: (tor_bridges) =>
    set((s) => ({ profile: { ...s.profile, tor_bridges } })),

  setPsiphonMode: (psiphon_mode) =>
    set((s) => ({ profile: { ...s.profile, psiphon_mode } })),

  setTorRelays: (tor_relays) => set((s) => ({ profile: { ...s.profile, tor_relays } })),
  setTorBridge: (tor_bridge) => set((s) => ({ profile: { ...s.profile, tor_bridge } })),
  setTorBridgeFile: (tor_bridge_file) =>
    set((s) => ({ profile: { ...s.profile, tor_bridge_file } })),
  setTorBind: (tor_bind) => set((s) => ({ profile: { ...s.profile, tor_bind } })),
  setPsiphonConfig: (psiphon_config) =>
    set((s) => ({ profile: { ...s.profile, psiphon_config } })),
  setPsiphonCdnIps: (psiphon_cdn_ips) =>
    set((s) => ({ profile: { ...s.profile, psiphon_cdn_ips } })),
  setPsiphonCdnSni: (psiphon_cdn_sni) =>
    set((s) => ({ profile: { ...s.profile, psiphon_cdn_sni } })),
  setPsiphonCdnSets: (psiphon_cdn_sets) =>
    set((s) => ({ profile: { ...s.profile, psiphon_cdn_sets } })),
  setPsiphonServerEntries: (psiphon_server_entries) =>
    set((s) => ({ profile: { ...s.profile, psiphon_server_entries } })),
  setPsiphonBind: (psiphon_bind) =>
    set((s) => ({ profile: { ...s.profile, psiphon_bind } })),

  setPeer: (peer) => set((s) => ({ profile: { ...s.profile, peer } })),
  setWgPeer: (wg_peer) => set((s) => ({ profile: { ...s.profile, wg_peer } })),
  setGoolPeer: (gool_peer) => set((s) => ({ profile: { ...s.profile, gool_peer } })),
  setWiwOuter: (wiw_outer) => set((s) => ({ profile: { ...s.profile, wiw_outer } })),
  setWiwInner: (wiw_inner) => set((s) => ({ profile: { ...s.profile, wiw_inner } })),
  setMimOuter: (mim_outer) => set((s) => ({ profile: { ...s.profile, mim_outer } })),
  setMimInner: (mim_inner) => set((s) => ({ profile: { ...s.profile, mim_inner } })),
  setExitLoc: (exit_loc) => set((s) => ({ profile: { ...s.profile, exit_loc } })),
  setApiFragment: (api_fragment) => set((s) => ({ profile: { ...s.profile, api_fragment } })),
  setNoQuicV2: (no_quic_v2) => set((s) => ({ profile: { ...s.profile, no_quic_v2 } })),
  setH2Peer: (h2_peer) => set((s) => ({ profile: { ...s.profile, h2_peer } })),
  setNoDataCheck: (no_data_check) => set((s) => ({ profile: { ...s.profile, no_data_check } })),
  setNoProfileRetry: (no_profile_retry) =>
    set((s) => ({ profile: { ...s.profile, no_profile_retry } })),
  setKeepalive: (keepalive) => set((s) => ({ profile: { ...s.profile, keepalive } })),
  setPerf: (perf) => set((s) => ({ profile: { ...s.profile, perf } })),
  setEch: (ech) => set((s) => ({ profile: { ...s.profile, ech } })),
  setEchDns: (ech_dns) => set((s) => ({ profile: { ...s.profile, ech_dns } })),
  setEchDomain: (ech_domain) => set((s) => ({ profile: { ...s.profile, ech_domain } })),
  setTlsCiphers: (tls_ciphers) => set((s) => ({ profile: { ...s.profile, tls_ciphers } })),
  setTlsGroups: (tls_groups) => set((s) => ({ profile: { ...s.profile, tls_groups } })),
  setDisableGrease: (disable_grease) =>
    set((s) => ({ profile: { ...s.profile, disable_grease } })),
  setMasqueMtu: (masque_mtu) => set((s) => ({ profile: { ...s.profile, masque_mtu } })),
  setNetstackRx: (netstack_rx) => set((s) => ({ profile: { ...s.profile, netstack_rx } })),
  setNetstackTx: (netstack_tx) => set((s) => ({ profile: { ...s.profile, netstack_tx } })),
  setRouteSniff: (route_sniff) => set((s) => ({ profile: { ...s.profile, route_sniff } })),

  clearLogs: () => set({ logs: [] }),

  // Clears the fallback screen so the user can attempt Connect again (e.g.
  // after fixing a broken install) — the next connect() call will re-set
  // sidecarError if the binary is still missing.
  retryAfterSidecarError: () => set({ sidecarError: null }),

  setManualMode: (on) => {
    set({ manualMode: on });
    persistManual();
  },

  addConfigs: (items) => {
    set((s) => {
      const ids = new Set(s.configs.map((c) => c.id));
      const fresh = items.filter((c) => !ids.has(c.id));
      const configs = [...s.configs, ...fresh];
      const activeConfigId = s.activeConfigId ?? fresh[0]?.id ?? null;
      return { configs, activeConfigId };
    });
    persistManual();
  },

  removeConfig: (id) => {
    set((s) => ({
      configs: s.configs.filter((c) => c.id !== id),
      activeConfigId: s.activeConfigId === id ? null : s.activeConfigId,
    }));
    persistManual();
  },

  setActiveConfig: (id) => {
    set({ activeConfigId: id });
    persistManual();
  },

  setConfigLatency: (id, ms) => {
    set((s) => ({
      configs: s.configs.map((c) => (c.id === id ? { ...c, latencyMs: ms } : c)),
    }));
    persistManual();
  },

  setConfigVia: (id, via) => {
    set((s) => ({
      configs: s.configs.map((c) => (c.id === id ? { ...c, via } : c)),
    }));
    persistManual();
  },
  };
});

// Dev-only: lets the 3D backdrop's per-state moods be driven from the WebView2
// devtools console without a live tunnel, e.g.
//   __conn.setState({ status: { state: "Connecting" } })
// Tree-shaken out of production builds by the import.meta.env.DEV guard.
if (import.meta.env.DEV) {
  (window as unknown as { __conn?: typeof useConnectionStore }).__conn = useConnectionStore;
}

const BUDGET_RE = /budget=(\d+)s/;
const TOR_PCT_RE = /reaching the network:\s*(\d+)%/;
// Core stats line: "[=] up 1.2 MiB down 34.5 MiB uptime 00:05:00"
const STATS_RE = /\[=\] up ([\d.]+) (\w+) down ([\d.]+) (\w+)/;

function toBytes(value: number, unit: string): number {
  switch (unit) {
    case "KiB": return value * 1024;
    case "MiB": return value * 1024 * 1024;
    case "GiB": return value * 1024 * 1024 * 1024;
    case "TiB": return value * 1024 * 1024 * 1024 * 1024;
    default: return value;
  }
}

/** While a matrix run is active, lane sessions emit global status/log
 * events that must NOT drive the main UI (status line, sysproxy
 * auto-apply, logs). Set by the Lab tab; leaving Lab aborts the run. */
let matrixActive = false;
export function setMatrixActive(on: boolean) {
  matrixActive = on;
}

/** Call once from App's top-level effect; returns a cleanup function. */
export async function initConnectionListeners(): Promise<() => void> {
  // Log lines arrive fast during route scanning; flushing to the store per
  // line would mean an O(logs) array copy + a re-render each. Coalesce into
  // one store write per ~100ms instead.
  let pendingLogs: LogLine[] = [];
  let flushTimer: ReturnType<typeof setTimeout> | null = null;
  const flushLogs = () => {
    flushTimer = null;
    const batch = pendingLogs;
    pendingLogs = [];
    let budget: number | null = null;
    let tor: number | null = null;
    let statUp: number | null = null;
    let statDown: number | null = null;
    for (const l of batch) {
      const m = BUDGET_RE.exec(l.line);
      if (m) budget = Number(m[1]);
      const t = TOR_PCT_RE.exec(l.line);
      if (t) tor = Number(t[1]);
      const s = STATS_RE.exec(l.line);
      if (s) {
        statUp = toBytes(Number(s[1]), s[2]);
        statDown = toBytes(Number(s[3]), s[4]);
      }
    }
    // Per-second rates from the delta since the previous stats line. The
    // core reports every ~2s (AETHER_STATS_SECS), so divide by wall time.
    let traffic: ConnectionState["traffic"] = null;
    let sample: ConnectionState["samples"][number] | null = null;
    if (statUp !== null && statDown !== null) {
      const prev = useConnectionStore.getState().traffic;
      const now = Date.now();
      const dt = prev && prev.at > 0 ? Math.max(1, (now - prev.at) / 1000) : 2;
      const upRate = Math.max(0, (statUp - (prev?.up ?? 0)) / dt);
      const downRate = Math.max(0, (statDown - (prev?.down ?? 0)) / dt);
      traffic = { up: statUp, down: statDown, upRate, downRate, at: now };
      sample = { t: now, down: downRate, up: upRate };
    }
    useConnectionStore.setState((s) => ({
      logs: [...s.logs, ...batch].slice(-MAX_LOG_LINES),
      ...(budget !== null ? { scanBudgetSecs: budget } : {}),
      ...(tor !== null ? { torPercent: tor } : {}),
      ...(traffic !== null ? { traffic } : {}),
      ...(sample !== null ? { samples: [...s.samples, sample].slice(-120) } : {}),
    }));
  };

  const [unlistenStatus, unlistenLog] = await Promise.all([
    listen<ConnectionStatus>("aether://status", (e) => {
      if (matrixActive) return;
      useConnectionStore.setState((s) => ({
        status: e.payload,
        // Fresh attempt — last attempt's budget/percent/counters reset.
        ...(e.payload.state === "Launching"
          ? { scanBudgetSecs: null, torPercent: null, traffic: null, samples: [] }
          : {}),
        // Disconnect gap marker so the graph line visibly breaks.
        ...(e.payload.state === "Idle" && s.samples.length > 0 && s.samples[s.samples.length - 1].down !== null
          ? { samples: [...s.samples, { t: Date.now(), down: null, up: null }].slice(-120) }
          : {}),
      }));
    }),
    listen<LogLine>("aether://log", (e) => {
      if (matrixActive) return;
      pendingLogs.push(e.payload);
      flushTimer ??= setTimeout(flushLogs, 100);
    }),
  ]);

  // Reconcile state in case the window reopened mid-session, and load the
  // last-successful profile so the protocol selector reflects it. Neither
  // command touches the Aether binary, so a failure here is an IPC-layer
  // bug, not a sidecar problem — logged rather than shown as sidecarError.
  try {
    const [status, profile] = await Promise.all([
      invoke<ConnectionStatus>("get_status"),
      invoke<ConnectionProfile>("get_default_profile"),
    ]);
    useConnectionStore.setState({ status, profile });
  } catch (e) {
    console.error("Failed to load initial connection state:", e);
  }

  return () => {
    unlistenStatus();
    unlistenLog();
    if (flushTimer !== null) clearTimeout(flushTimer);
  };
}
