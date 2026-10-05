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
}

export const useConnectionStore = create<ConnectionState>((set, get) => ({
  status: { state: "Idle" },
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
}));

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
