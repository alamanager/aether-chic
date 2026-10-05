// Mirrors src-tauri/src/state.rs::ConnectionState (serde adjacently-tagged
// via `#[serde(tag = "state")]`) and src-tauri/src/aether/profiles.rs.

export type ConnectionStatus =
  | { state: "Idle" }
  | { state: "Launching" }
  | { state: "Connecting" }
  | { state: "Connected"; socks_addr: string; connected_at_ms: number }
  | { state: "Reconnecting"; attempt: number; max_attempts: number }
  | { state: "Disconnecting" }
  | { state: "Error"; message: string; phase: string };

export type Protocol = "auto" | "masque" | "wireguard" | "gool" | "gool_classic" | "mim";
export type ScanMode = "turbo" | "balanced" | "thorough" | "verified" | "ironclad";
export type IpVersion = "v4" | "v6" | "both";
export type Noize = "off" | "light" | "firewall" | "balanced" | "gfw" | "aggressive";
export type ZeroTrustAuth = "email" | "service" | "token";
export type ExtraTransport =
  | "none"
  | "tor"
  | "tor_reverse"
  | "tor_only"
  | "psiphon"
  | "psiphon_reverse"
  | "psiphon_only";

export interface ConnectionProfile {
  protocol: Protocol;
  scan_mode: ScanMode;
  ip_version: IpVersion;
  /** Aether ≥1.1.1: reuse the last known-working gateway with a quick
   * recheck instead of a full scan. */
  quick_reconnect: boolean;
  /** Aether ≥1.2.0: run MASQUE over HTTP/2 (TCP) instead of the default
   * HTTP/3 (QUIC) — for networks that block or throttle UDP. */
  masque_http2: boolean;
  /** Unified obfuscation profile (v2.2 one list for every protocol). */
  noize: Noize;
  /** Local SOCKS5 listen address (--bind). Default 127.0.0.1:1819. */
  bind_address: string;
  /** Custom HTTP proxy port (empty = SOCKS port + 1). */
  http_port: string;
  /** Aether ≥1.5.0: optional comma-separated DNS resolvers inside the tunnel. */
  dns: string;
  /** Cloudflare Zero Trust team. Empty keeps the normal consumer WARP flow. */
  zero_trust_team: string;
  zero_trust_auth: ZeroTrustAuth;
  /** Zero Trust credentials stay in memory only; they are never persisted. */
  access_email: string;
  access_client_id: string;
  access_client_secret: string;
  access_token: string;
  /** Route HTTP/HTTPS through the organization's Gateway proxy. */
  zero_trust_gateway: boolean;
  /** Aether ≥1.5.0 traffic-routing rules. */
  route_block: string;
  /** Aether ≥1.7.0: chain out through another proxy/VPN app. */
  upstream: string;
  /** Aether ≥2.0/2.1: built-in Tor / Psiphon transports (needs core's pt/). */
  extra_transport: ExtraTransport;
  /** Psiphon egress region (ISO alpha-2, "" = automatic). */
  psiphon_region: string;
  /** v2.2 forced peers (empty = scan). */
  peer: string;
  wg_peer: string;
  gool_peer: string;
  wiw_outer: string;
  wiw_inner: string;
  mim_outer: string;
  mim_inner: string;
  /** v2.2 exit-country enforcement, e.g. "!IR,AZ,RU" (empty = off). */
  exit_loc: string;
  /** v2.2 MASQUE/TLS toggles. */
  api_fragment: boolean;
  no_quic_v2: boolean;
  h2_peer: string;
  no_data_check: boolean;
  no_profile_retry: boolean;
  keepalive: string;
  perf: string;
  /** v2.2 ECH ("" = off, "auto" or base64 key) + lookup knobs. */
  ech: string;
  ech_dns: string;
  ech_domain: string;
  /** v2.2 TLS fingerprint (empty = Chrome defaults). */
  tls_ciphers: string;
  tls_groups: string;
  disable_grease: boolean;
  /** v2.2 tuning env knobs (empty = core defaults). */
  masque_mtu: string;
  netstack_rx: string;
  netstack_tx: string;
  route_sniff: boolean;
  route_direct: string;
  routes_file: string;
}

export interface LogLine {
  line: string;
  timestamp: number;
}
