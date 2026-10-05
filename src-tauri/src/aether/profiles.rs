use serde::{Deserialize, Serialize};

/// `Auto` resolves to Aether's own default (MASQUE). Aether's own `scan_mode`
/// already performs multi-route discovery internally (confirmed by manually
/// running the real binary), so Aether-GUI does not implement a client-side
/// protocol-fallback retry loop on top of this.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Auto,
    Masque,
    Wireguard,
    Gool,
    GoolClassic,
    Mim,
}

impl Protocol {
    /// The literal menu choice Aether expects at its "Protocol:" prompt.
    /// GoolClassic/Mim map to "3" by analogy only (v2.2 menu order
    /// unverified) — a pure fallback, since flags suppress menus anyway.
    pub fn as_menu_choice(&self) -> &'static str {
        match self {
            Protocol::Auto | Protocol::Masque => "1",
            Protocol::Wireguard => "2",
            Protocol::Gool | Protocol::GoolClassic | Protocol::Mim => "3",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ScanMode {
    Turbo,
    Balanced,
    Thorough,
    Verified,
    Ironclad,
}

impl ScanMode {
    pub fn as_menu_choice(&self) -> &'static str {
        match self {
            ScanMode::Turbo => "1",
            ScanMode::Balanced => "2",
            ScanMode::Thorough => "3",
            ScanMode::Verified => "4",
            ScanMode::Ironclad => "5",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum IpVersion {
    V4,
    V6,
    Both,
}

impl IpVersion {
    pub fn as_menu_choice(&self) -> &'static str {
        match self {
            IpVersion::V4 => "1",
            IpVersion::V6 => "2",
            IpVersion::Both => "3",
        }
    }
}

/// Obfuscation profile, one list for every protocol since core v2.2
/// (`off | light | firewall | balanced | gfw | aggressive`; firewall is
/// the MASQUE default, balanced for the rest). Passed as `--noize`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Noize {
    Off,
    Light,
    #[default]
    Firewall,
    Balanced,
    Gfw,
    Aggressive,
}

impl Noize {
    pub fn as_flag(&self) -> &'static str {
        match self {
            Noize::Off => "off",
            Noize::Light => "light",
            Noize::Firewall => "firewall",
            Noize::Balanced => "balanced",
            Noize::Gfw => "gfw",
            Noize::Aggressive => "aggressive",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ConnectionProfile {    pub protocol: Protocol,
    pub scan_mode: ScanMode,
    pub ip_version: IpVersion,
    /// Aether ≥1.1.1: reuse the last known-working gateway with a quick
    /// recheck instead of a full scan. `serde(default)` keeps profiles saved
    /// by older versions of this app loading cleanly.
    #[serde(default = "default_true")]
    pub quick_reconnect: bool,
    /// Aether ≥1.2.0: run the MASQUE tunnel over HTTP/2 (TCP) instead of the
    /// default HTTP/3 (QUIC) — for networks that block or throttle UDP.
    /// Passed as the AETHER_MASQUE_HTTP2 env var, not a flag: there is no
    /// `--h3` flag, and setting the env to any value also suppresses 1.2.0's
    /// new interactive "MASQUE transport" prompt in both directions.
    #[serde(default)]
    pub masque_http2: bool,
    /// Unified obfuscation profile since core v2.2 (one list for every
    /// protocol). Passed as `--noize <value>`.
    #[serde(default)]
    pub noize: Noize,
    /// Local SOCKS5 listen address (`--bind`). Aether defaults to
    /// 127.0.0.1:1819; users can change the port or bind to 0.0.0.0 for LAN.
    #[serde(default = "default_bind_address")]
    pub bind_address: String,
    /// Custom HTTP proxy port (empty = SOCKS port + 1). Same IP as --bind,
    /// so LAN sharing covers both endpoints at once.
    #[serde(default)]
    pub http_port: String,
    /// Aether ≥1.5.0: optional resolvers used *inside* the tunnel. Kept as
    /// Aether's comma-separated CLI format, for example `1.1.1.1,1.0.0.1`.
    #[serde(default)]
    pub dns: String,
    /// Aether ≥1.5.0: Cloudflare Zero Trust organization name. An empty
    /// value means the normal consumer WARP flow.
    #[serde(default)]
    pub zero_trust_team: String,
    /// Which Zero Trust credential field is active in the GUI. This controls
    /// what is handed to the core, rather than being a core flag itself.
    #[serde(default)]
    pub zero_trust_auth: ZeroTrustAuth,
    /// Email used for Cloudflare Access one-time-code sign-in. Sensitive
    /// values are erased before the successful profile is persisted.
    #[serde(default)]
    pub access_email: String,
    /// Cloudflare Access service-token client id.
    #[serde(default)]
    pub access_client_id: String,
    /// Cloudflare Access service-token secret.
    #[serde(default)]
    pub access_client_secret: String,
    /// A pre-obtained Cloudflare Access enrolment JWT.
    #[serde(default)]
    pub access_token: String,
    /// Route HTTP/HTTPS through the organization's Gateway proxy. This is
    /// intentionally off by default because the organization can log it.
    #[serde(default)]
    pub zero_trust_gateway: bool,
    /// Aether ≥1.5.0 routing lists. Entries are comma/newline separated in
    /// the same format accepted by `--route-block` and `--route-direct`.
    #[serde(default)]
    pub route_block: String,
    #[serde(default)]
    pub route_direct: String,
    /// Optional path to an Aether routing file with [block]/[direct] sections.
    #[serde(default)]
    pub routes_file: String,
    /// v2.2 forced peers (skip scanning for named hops): MASQUE/WireGuard
    /// peer, WireGuard peer (warp-in-warp outer), gool inner peer.
    #[serde(default)]
    pub peer: String,
    #[serde(default)]
    pub wg_peer: String,
    #[serde(default)]
    pub gool_peer: String,
    /// v2.2 classic-gool / mim manual endpoints (port required).
    #[serde(default)]
    pub wiw_outer: String,
    #[serde(default)]
    pub wiw_inner: String,
    #[serde(default)]
    pub mim_outer: String,
    #[serde(default)]
    pub mim_inner: String,
    /// v2.2 exit-country enforcement, e.g. "!IR,AZ,RU" or "DE,SE". Empty = off.
    #[serde(default)]
    pub exit_loc: String,
    /// v2.2 flags: fragment the API route, skip QUIC v2 opener, H2 peer,
    /// skip data-plane validation, profile retry, keepalive, perf profile.
    #[serde(default)]
    pub api_fragment: bool,
    #[serde(default)]
    pub no_quic_v2: bool,
    #[serde(default)]
    pub h2_peer: String,
    #[serde(default)]
    pub no_data_check: bool,
    #[serde(default)]
    pub no_profile_retry: bool,
    #[serde(default)]
    pub keepalive: String,
    #[serde(default)]
    pub perf: String,
    /// v2.2 ECH (empty = off, "auto" or base64 key) plus its lookup knobs.
    #[serde(default)]
    pub ech: String,
    #[serde(default)]
    pub ech_dns: String,
    #[serde(default)]
    pub ech_domain: String,
    /// v2.2 TLS fingerprint controls (empty = Chrome defaults).
    #[serde(default)]
    pub tls_ciphers: String,
    #[serde(default)]
    pub tls_groups: String,
    #[serde(default)]
    pub disable_grease: bool,
    /// v2.2 tuning env knobs (empty = core defaults): inner MTU, netstack
    /// TCP buffers, SNI sniffing (route_sniff off writes AETHER_ROUTE_SNIFF=0).
    #[serde(default)]
    pub masque_mtu: String,
    #[serde(default)]
    pub netstack_rx: String,
    #[serde(default)]
    pub netstack_tx: String,
    #[serde(default = "default_true")]
    pub route_sniff: bool,
    /// v2.2 Tor extras: relays mode (auto/only/off/count), custom bridge
    /// lines (one per line), bridge file, and the Tor exit listener.
    #[serde(default)]
    pub tor_relays: String,
    #[serde(default)]
    pub tor_bridge: String,
    #[serde(default)]
    pub tor_bridge_file: String,
    #[serde(default)]
    pub tor_bind: String,
    /// v2.2 Psiphon extras: custom config overlay, CDN fronting lists,
    /// seed server entries, and the Psiphon exit listener.
    #[serde(default)]
    pub psiphon_config: String,
    #[serde(default)]
    pub psiphon_cdn_ips: String,
    #[serde(default)]
    pub psiphon_cdn_sni: String,
    #[serde(default)]
    pub psiphon_cdn_sets: String,
    #[serde(default)]
    pub psiphon_server_entries: String,
    #[serde(default)]
    pub psiphon_bind: String,
    /// Aether ≥1.7.0: dial out through another proxy (chain behind a VPN or
    /// proxy app already on the machine). Accepts socks5://, http:// or a
    /// bare host:port (SOCKS5), with credentials in the URL.
    #[serde(default)]
    pub upstream: String,
    /// Aether ≥2.0/2.1: built-in Tor / Psiphon transports. Exactly one mode
    /// is ever passed (the CLI flags are mutually exclusive), so this is a
    /// single enum rather than independent toggles that could combine badly.
    #[serde(default)]
    pub extra_transport: ExtraTransport,
    /// Skip Tor's plain attempt and go straight to bridges (core tries ~75s
    /// of direct first, which is futile on a network that blocks Tor —
    /// observed stuck fetching consensus). Only forwarded with a Tor mode.
    #[serde(default)]
    pub tor_bridges: bool,
    /// Psiphon egress region (ISO alpha-2, e.g. "DE"). Empty = automatic.
    /// Only forwarded with a Psiphon mode; the core treats it as a hard
    /// filter, so a region with no current exit won't connect — retry Auto.
    #[serde(default)]
    pub psiphon_region: String,
    /// Psiphon shape: automatic, fronted-meek-only (cdn, for networks that
    /// block the rest) or no fronting (direct).
    #[serde(default)]
    pub psiphon_mode: PsiphonMode,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PsiphonMode {
    #[default]
    Auto,
    Cdn,
    Direct,
}

impl PsiphonMode {
    pub fn as_flag(&self) -> Option<&'static str> {
        match self {
            PsiphonMode::Auto => None,
            PsiphonMode::Cdn => Some("cdn"),
            PsiphonMode::Direct => Some("direct"),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExtraTransport {
    #[default]
    None,
    Tor,
    TorReverse,
    TorOnly,
    Psiphon,
    PsiphonReverse,
    PsiphonOnly,
    /// Direct console-client mode (psiphon_direct.rs): bypasses the Aether
    /// core entirely. Produces NO core flags — the module drives
    /// pt/psiphon-tunnel-core itself.
    PsiphonDirect,
}

impl ExtraTransport {
    /// The literal CLI flag, if any. Needs the v2.x core plus its `pt/`
    /// transports folder next to the binary (bundled since the v2.1.0 pin).
    pub fn as_flag(&self) -> Option<&'static str> {
        match self {
            ExtraTransport::None => None,
            ExtraTransport::Tor => Some("--tor"),
            ExtraTransport::TorReverse => Some("--tor-reverse"),
            ExtraTransport::TorOnly => Some("--tor-only"),
            ExtraTransport::Psiphon => Some("--psiphon"),
            ExtraTransport::PsiphonReverse => Some("--psiphon-reverse"),
            ExtraTransport::PsiphonOnly => Some("--psiphon-only"),
            ExtraTransport::PsiphonDirect => None,
        }
    }

    pub fn is_direct(&self) -> bool {
        matches!(self, ExtraTransport::PsiphonDirect)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ZeroTrustAuth {
    #[default]
    Email,
    Service,
    Token,
}

fn default_true() -> bool {
    true
}

fn default_bind_address() -> String {
    "127.0.0.1:1819".into()
}

/// HTTP endpoint served by the core next to SOCKS5: same IP, custom port
/// when set and sane, else SOCKS port + 1. Falls back to 127.0.0.1:1820
/// when the bind address doesn't parse.
pub fn http_proxy_addr(bind_address: &str, http_port: &str) -> String {
    http_proxy_socket(bind_address, http_port).to_string()
}

/// Socket version of the above. An unspecified (0.0.0.0) bind maps to
/// loopback — see status::client_addr — because 0.0.0.0 is not connectable
/// and must never be reported to clients or the system proxy.
pub fn http_proxy_socket(bind_address: &str, http_port: &str) -> std::net::SocketAddr {
    match bind_address.parse::<std::net::SocketAddr>() {
        Ok(socks) => {
            let mut http = super::status::client_addr(&socks);
            // A custom port wins unless it is invalid or collides with the
            // SOCKS port itself (the core could never bind both).
            if let Ok(p) = http_port.trim().parse::<u16>() {
                if p >= 1 && p != socks.port() {
                    http.set_port(p);
                    return http;
                }
            }
            http.set_port(socks.port().wrapping_add(1));
            if http.port() == 0 {
                http.set_port(1820);
            }
            http
        }
        Err(_) => "127.0.0.1:1820"
            .parse()
            .expect("loopback literal always parses"),
    }
}

/// Effective Tor exit listener for inside/reverse modes: explicit field or
/// core default. None otherwise (tor-only serves on --bind instead).
pub fn tor_exit_addr(p: &ConnectionProfile) -> Option<std::net::SocketAddr> {
    if !matches!(
        p.extra_transport,
        ExtraTransport::Tor | ExtraTransport::TorReverse
    ) {
        return None;
    }
    let raw = if p.tor_bind.trim().is_empty() {
        "127.0.0.1:1820".to_string()
    } else {
        p.tor_bind.trim().to_string()
    };
    raw.parse::<std::net::SocketAddr>()
        .ok()
        .map(|s| super::status::client_addr(&s))
}

/// Effective Psiphon exit listener for inside/reverse modes.
pub fn psi_exit_addr(p: &ConnectionProfile) -> Option<std::net::SocketAddr> {
    if !matches!(
        p.extra_transport,
        ExtraTransport::Psiphon | ExtraTransport::PsiphonReverse
    ) {
        return None;
    }
    let raw = if p.psiphon_bind.trim().is_empty() {
        "127.0.0.1:1821".to_string()
    } else {
        p.psiphon_bind.trim().to_string()
    };
    raw.parse::<std::net::SocketAddr>()
        .ok()
        .map(|s| super::status::client_addr(&s))
}

/// Display HTTP endpoint: base address, bumped while it collides with the
/// SOCKS port or a Tor/Psiphon exit listener. Ports overlap across bind IPs
/// (0.0.0.0:1820 and 127.0.0.1:1820 collide), so the comparison is by port —
/// e.g. tor-inside with everything default puts both on 1820 and HTTP
/// moves aside; the log reports the real ports.
pub fn http_proxy_socket_for(p: &ConnectionProfile) -> std::net::SocketAddr {
    let socks_port = p
        .bind_address
        .parse::<std::net::SocketAddr>()
        .map(|s| s.port())
        .unwrap_or(1819);
    let mut http = http_proxy_socket(&p.bind_address, &p.http_port);
    let tor = tor_exit_addr(p).map(|s| s.port());
    let psi = psi_exit_addr(p).map(|s| s.port());
    for _ in 0..3 {
        if http.port() != socks_port && Some(http.port()) != tor && Some(http.port()) != psi {
            break;
        }
        http.set_port(http.port().wrapping_add(1));
        if http.port() == 0 {
            http.set_port(1820);
        }
    }
    http
}

/// The --http-proxy flag value: same resolved port as the display address,
/// but on the bind IP (so LAN sharing covers HTTP too).
pub fn flag_http_addr_for(p: &ConnectionProfile) -> String {
    let port = http_proxy_socket_for(p).port();
    match p.bind_address.parse::<std::net::SocketAddr>() {
        Ok(socks) => std::net::SocketAddr::new(socks.ip(), port).to_string(),
        Err(_) => format!("127.0.0.1:{port}"),
    }
}

impl ConnectionProfile {
    /// CLI flags for Aether ≥1.1.1 — the whole profile is passed up front so
    /// the interactive prompts never appear (the PTY prompt-answering in
    /// pty.rs stays as a fallback). One of the two quick-reconnect flags is
    /// ALWAYS passed: without either, 1.1.1 asks its own interactive
    /// "reconnect with last gateway?" question, which the GUI must never
    /// leave unanswered.
    pub fn as_args(&self) -> Vec<String> {
        let mut args = Vec::with_capacity(20);
        match self.protocol {
            Protocol::Auto => {}
            Protocol::Masque => args.push("--masque".into()),
            Protocol::Wireguard => args.push("--wg".into()),
            Protocol::Gool => args.push("--gool".into()),
            Protocol::GoolClassic => args.push("--gool-classic".into()),
            Protocol::Mim => args.push("--mim".into()),
        }
        args.push(match self.scan_mode {
            ScanMode::Turbo => "--turbo".into(),
            ScanMode::Balanced => "--balanced".into(),
            ScanMode::Thorough => "--thorough".into(),
            ScanMode::Verified => "--verified".into(),
            ScanMode::Ironclad => "--ironclad".into(),
        });
        args.push(match self.ip_version {
            IpVersion::V4 => "-4".into(),
            IpVersion::V6 => "-6".into(),
            IpVersion::Both => "--dual".into(),
        });
        args.push(if self.quick_reconnect {
            "--quick-reconnect".into()
        } else {
            "--no-quick-reconnect".into()
        });
        // Unified obfuscation profile (v2.2 one list for every protocol).
        args.push("--noize".into());
        args.push(self.noize.as_flag().into());
        // Only forward --bind when non-default and parseable.
        if self.bind_address != default_bind_address()
            && self.bind_address.parse::<std::net::SocketAddr>().is_ok()
        {
            args.push("--bind".into());
            args.push(self.bind_address.clone());
        }
        // Aether ≥1.6.0 serves a native HTTP CONNECT proxy next to SOCKS5 —
        // this replaced the GUI's old hand-rolled bridge. Always on, on its
        // own port (custom or SOCKS port + 1) so the two stay separable.
        // NOTE: the flag keeps the bind IP (LAN sharing covers HTTP too);
        // only the displayed/proxied address maps 0.0.0.0 to loopback.
        let http_addr = flag_http_addr_for(self);
        args.push("--http-proxy".into());
        args.push(http_addr);
        if !self.upstream.trim().is_empty() {
            args.push("--upstream".into());
            args.push(self.upstream.trim().into());
        }
        if let Some(flag) = self.extra_transport.as_flag() {
            args.push(flag.into());
        }
        // Gated on a Tor mode: without one the flag is meaningless and the
        // core might reject it.
        if self.tor_bridges
            && matches!(
                self.extra_transport,
                ExtraTransport::Tor | ExtraTransport::TorReverse | ExtraTransport::TorOnly
            )
        {
            args.push("--tor-bridges".into());
        }
        // Region/mode only make sense with a Psiphon mode active.
        if matches!(
            self.extra_transport,
            ExtraTransport::Psiphon | ExtraTransport::PsiphonReverse | ExtraTransport::PsiphonOnly
        ) {
            let region = self.psiphon_region.trim().to_uppercase();
            if !region.is_empty() {
                args.push("--psiphon-region".into());
                args.push(region);
            }
            if let Some(mode) = self.psiphon_mode.as_flag() {
                args.push("--psiphon-mode".into());
                args.push(mode.into());
            }
        }
        if !self.dns.trim().is_empty() {
            args.push("--dns".into());
            args.push(self.dns.trim().into());
        }
        if !self.zero_trust_team.trim().is_empty() {
            args.push("--team".into());
            args.push(self.zero_trust_team.trim().into());
            if self.zero_trust_gateway {
                args.push("--gateway".into());
            }
        }
        if !self.route_block.trim().is_empty() {
            args.push("--route-block".into());
            args.push(self.route_block.trim().into());
        }
        if !self.route_direct.trim().is_empty() {
            args.push("--route-direct".into());
            args.push(self.route_direct.trim().into());
        }
        if !self.routes_file.trim().is_empty() {
            args.push("--routes".into());
            args.push(self.routes_file.trim().into());
        }
        // v2.2 forced/manual endpoints (skip scanning for named hops).
        for (flag, val) in [
            ("--peer", &self.peer),
            ("--wg-peer", &self.wg_peer),
            ("--gool-peer", &self.gool_peer),
            ("--wiw-outer", &self.wiw_outer),
            ("--wiw-inner", &self.wiw_inner),
            ("--mim-outer", &self.mim_outer),
            ("--mim-inner", &self.mim_inner),
        ] {
            if !val.trim().is_empty() {
                args.push(flag.into());
                args.push(val.trim().into());
            }
        }
        if !self.exit_loc.trim().is_empty() {
            args.push("--exit-loc".into());
            args.push(self.exit_loc.trim().into());
        }
        if self.api_fragment {
            args.push("--api-fragment".into());
        }
        if self.no_quic_v2 {
            args.push("--no-quic-v2".into());
        }
        if !self.h2_peer.trim().is_empty() {
            args.push("--h2-peer".into());
            args.push(self.h2_peer.trim().into());
        }
        if self.no_data_check {
            args.push("--no-data-check".into());
        }
        if self.no_profile_retry {
            args.push("--no-profile-retry".into());
        }
        if !self.keepalive.trim().is_empty() {
            args.push("--keepalive".into());
            args.push(self.keepalive.trim().into());
        }
        if !self.perf.trim().is_empty() {
            args.push("--perf".into());
            args.push(self.perf.trim().into());
        }
        if !self.ech.trim().is_empty() {
            args.push("--ech".into());
            args.push(self.ech.trim().into());
        }
        if !self.ech_dns.trim().is_empty() {
            args.push("--ech-dns".into());
            args.push(self.ech_dns.trim().into());
        }
        if !self.ech_domain.trim().is_empty() {
            args.push("--ech-domain".into());
            args.push(self.ech_domain.trim().into());
        }
        if !self.tls_ciphers.trim().is_empty() {
            args.push("--tls-ciphers".into());
            args.push(self.tls_ciphers.trim().into());
        }
        if !self.tls_groups.trim().is_empty() {
            args.push("--tls-groups".into());
            args.push(self.tls_groups.trim().into());
        }
        if self.disable_grease {
            args.push("--disable-grease".into());
        }
        // Extra Tor controls (relays/bridges/bind), each gated on a Tor
        // mode like --tor-bridges above.
        let tor_mode = matches!(
            self.extra_transport,
            ExtraTransport::Tor | ExtraTransport::TorReverse | ExtraTransport::TorOnly
        );
        if tor_mode && !self.tor_relays.trim().is_empty() {
            args.push("--tor-relays".into());
            args.push(self.tor_relays.trim().into());
        }
        if tor_mode {
            for line in self.tor_bridge.lines().map(str::trim).filter(|l| !l.is_empty()) {
                args.push("--tor-bridge".into());
                args.push(line.into());
            }
            if !self.tor_bridge_file.trim().is_empty() {
                args.push("--tor-bridge-file".into());
                args.push(self.tor_bridge_file.trim().into());
            }
            if !self.tor_bind.trim().is_empty() {
                args.push("--tor-bind".into());
                args.push(self.tor_bind.trim().into());
            }
        }
        // Extra Psiphon controls, gated on a core Psiphon mode (direct
        // mode drives the console client itself — see psiphon_direct.rs).
        let psi_mode = matches!(
            self.extra_transport,
            ExtraTransport::Psiphon | ExtraTransport::PsiphonReverse | ExtraTransport::PsiphonOnly
        );
        if psi_mode {
            for (flag, val) in [
                ("--psiphon-config", &self.psiphon_config),
                ("--psiphon-cdn-ips", &self.psiphon_cdn_ips),
                ("--psiphon-cdn-sni", &self.psiphon_cdn_sni),
                ("--psiphon-cdn-sets", &self.psiphon_cdn_sets),
                ("--psiphon-server-entries", &self.psiphon_server_entries),
                ("--psiphon-bind", &self.psiphon_bind),
            ] {
                if !val.trim().is_empty() {
                    args.push(flag.into());
                    args.push(val.trim().into());
                }
            }
        }
        args
    }

    /// The core accepts Zero Trust credentials as flags too, but putting a
    /// JWT or service secret in the process command line exposes it to other
    /// local processes. pty.rs supplies the selected credential as an env var
    /// instead, and this method ensures only that one method is ever sent.
    pub fn zero_trust_env(&self) -> Option<(&'static str, &str)> {
        if self.zero_trust_team.trim().is_empty() {
            return None;
        }
        match self.zero_trust_auth {
            ZeroTrustAuth::Email if !self.access_email.trim().is_empty() => {
                Some(("AETHER_ACCESS_EMAIL", self.access_email.trim()))
            }
            ZeroTrustAuth::Service
                if !self.access_client_id.trim().is_empty()
                    && !self.access_client_secret.trim().is_empty() =>
            {
                // The id and secret need separate variables, so this method
                // cannot represent service credentials. pty.rs handles that
                // pair directly after consulting `zero_trust_auth`.
                None
            }
            ZeroTrustAuth::Token if !self.access_token.trim().is_empty() => {
                Some(("AETHER_ACCESS_TOKEN", self.access_token.trim()))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_omits_bind_flag() {
        let p = ConnectionProfile::default();
        let args = p.as_args();
        assert!(!args.iter().any(|a| a == "--bind"), "args={args:?}");
    }

    #[test]
    fn custom_port_emits_bind() {
        let mut p = ConnectionProfile::default();
        p.bind_address = "127.0.0.1:1919".into();
        let args = p.as_args();
        let i = args
            .iter()
            .position(|a| a == "--bind")
            .expect("missing --bind");
        assert_eq!(args.get(i + 1).map(String::as_str), Some("127.0.0.1:1919"));
    }

    #[test]
    fn lan_bind_emits_bind() {
        let mut p = ConnectionProfile::default();
        p.bind_address = "0.0.0.0:1819".into();
        let args = p.as_args();
        let i = args
            .iter()
            .position(|a| a == "--bind")
            .expect("missing --bind");
        assert_eq!(args.get(i + 1).map(String::as_str), Some("0.0.0.0:1819"));
    }

    #[test]
    fn lan_with_custom_port_emits_bind() {
        let mut p = ConnectionProfile::default();
        p.bind_address = "0.0.0.0:9999".into();
        let args = p.as_args();
        let i = args
            .iter()
            .position(|a| a == "--bind")
            .expect("missing --bind");
        assert_eq!(args.get(i + 1).map(String::as_str), Some("0.0.0.0:9999"));
    }

    #[test]
    fn invalid_bind_is_not_forwarded() {
        let mut p = ConnectionProfile::default();
        p.bind_address = "127.0.0.1:".into();
        let args = p.as_args();
        assert!(!args.iter().any(|a| a == "--bind"), "args={args:?}");
    }

    #[test]
    fn old_profile_json_gets_defaults() {
        let json = r#"{"protocol":"auto","scan_mode":"balanced","ip_version":"v4","quick_reconnect":true,"masque_http2":false}"#;
        let p: ConnectionProfile = serde_json::from_str(json).unwrap();
        assert_eq!(p.bind_address, "127.0.0.1:1819");
        assert_eq!(p.noize, Noize::Firewall);
        assert!(p.route_sniff);
        assert_eq!(p.extra_transport, ExtraTransport::None);
    }

    #[test]
    fn default_emits_noize() {
        let p = ConnectionProfile::default();
        let args = p.as_args();
        let i = args
            .iter()
            .position(|a| a == "--noize")
            .expect("missing --noize");
        assert_eq!(args.get(i + 1).map(String::as_str), Some("firewall"));
    }

    #[test]
    fn v150_options_emit_without_credentials() {
        let p = ConnectionProfile {
            dns: "9.9.9.9,1.1.1.1".into(),
            zero_trust_team: "acme".into(),
            zero_trust_gateway: true,
            route_block: "ads.example".into(),
            route_direct: "private".into(),
            routes_file: "C:/routes.txt".into(),
            ..Default::default()
        };
        assert_eq!(
            p.as_args(),
            vec![
                "--balanced",
                "-4",
                "--quick-reconnect",
                "--noize",
                "firewall",
                "--http-proxy",
                "127.0.0.1:1820",
                "--dns",
                "9.9.9.9,1.1.1.1",
                "--team",
                "acme",
                "--gateway",
                "--route-block",
                "ads.example",
                "--route-direct",
                "private",
                "--routes",
                "C:/routes.txt"
            ]
        );
    }

    #[test]
    fn http_proxy_follows_socks_port() {
        assert_eq!(http_proxy_addr("127.0.0.1:1819", ""), "127.0.0.1:1820");
        assert_eq!(http_proxy_addr("0.0.0.0:1919", ""), "127.0.0.1:1920");
        assert_eq!(http_proxy_addr("garbage", ""), "127.0.0.1:1820");
        let p = ConnectionProfile::default();
        let args = p.as_args();
        let i = args.iter().position(|a| a == "--http-proxy").expect("missing --http-proxy");
        assert_eq!(args.get(i + 1).map(String::as_str), Some("127.0.0.1:1820"));
    }

    #[test]
    fn custom_http_port_wins_unless_colliding() {
        assert_eq!(http_proxy_addr("127.0.0.1:1819", "8080"), "127.0.0.1:8080");
        // Same as SOCKS or invalid → falls back to SOCKS+1.
        assert_eq!(http_proxy_addr("127.0.0.1:1819", "1819"), "127.0.0.1:1820");
        assert_eq!(http_proxy_addr("127.0.0.1:1819", "abc"), "127.0.0.1:1820");
        // Display maps LAN binds to loopback; the core flag keeps them.
        assert_eq!(http_proxy_addr("0.0.0.0:1819", ""), "127.0.0.1:1820");
        let mut q = ConnectionProfile::default();
        q.bind_address = "0.0.0.0:1819".into();
        assert_eq!(flag_http_addr_for(&q), "0.0.0.0:1820");
        q.bind_address = "0.0.0.0:1919".into();
        q.http_port = "18080".into();
        assert_eq!(flag_http_addr_for(&q), "0.0.0.0:18080");
        let mut p = ConnectionProfile::default();
        p.http_port = "18080".into();
        let args = p.as_args();
        let i = args.iter().position(|a| a == "--http-proxy").expect("missing --http-proxy");
        assert_eq!(args.get(i + 1).map(String::as_str), Some("127.0.0.1:18080"));
    }

    #[test]
    fn upstream_and_extra_transport_emit() {
        let mut p = ConnectionProfile::default();
        p.upstream = "socks5://127.0.0.1:1080".into();
        p.extra_transport = ExtraTransport::Tor;
        let args = p.as_args();
        let i = args.iter().position(|a| a == "--upstream").expect("missing --upstream");
        assert_eq!(args.get(i + 1).map(String::as_str), Some("socks5://127.0.0.1:1080"));
        assert!(args.iter().any(|a| a == "--tor"));
        assert!(!args.iter().any(|a| a == "--psiphon"));
    }

    #[test]
    fn no_extra_transport_by_default() {
        let p = ConnectionProfile::default();
        let args = p.as_args();
        assert!(!args.iter().any(|a| a == "--tor" || a == "--psiphon"));
        assert!(!args.iter().any(|a| a == "--upstream"));
    }

    #[test]
    fn psiphon_region_and_mode_gated() {
        let mut p = ConnectionProfile::default();
        p.extra_transport = ExtraTransport::PsiphonOnly;
        p.psiphon_region = "de".into();
        p.psiphon_mode = PsiphonMode::Cdn;
        let args = p.as_args();
        let i = args.iter().position(|a| a == "--psiphon-region").expect("missing region");
        assert_eq!(args.get(i + 1).map(String::as_str), Some("DE"));
        let j = args.iter().position(|a| a == "--psiphon-mode").expect("missing mode");
        assert_eq!(args.get(j + 1).map(String::as_str), Some("cdn"));
        // Without a Psiphon mode neither flag is forwarded.
        p.extra_transport = ExtraTransport::TorOnly;
        let args = p.as_args();
        assert!(!args.iter().any(|a| a == "--psiphon-region" || a == "--psiphon-mode"));
    }

    #[test]
    fn psiphon_direct_emits_no_core_flags() {
        let mut p = ConnectionProfile::default();
        p.extra_transport = ExtraTransport::PsiphonDirect;
        p.psiphon_region = "DE".into();
        p.psiphon_mode = PsiphonMode::Cdn;
        let args = p.as_args();
        // The direct console client is driven by its own module, never by
        // core flags — not even region/mode.
        assert!(!args.iter().any(|a| a == "--psiphon"
            || a == "--psiphon-only"
            || a == "--psiphon-region"
            || a == "--psiphon-mode"));
        assert!(args.iter().any(|a| a == "--http-proxy"));
    }

    #[test]
    fn tor_bridges_only_with_tor_mode() {
        let mut p = ConnectionProfile::default();
        p.extra_transport = ExtraTransport::TorOnly;
        p.tor_bridges = true;
        let args = p.as_args();
        assert!(args.iter().any(|a| a == "--tor-bridges"));
        p.extra_transport = ExtraTransport::PsiphonOnly;
        let args = p.as_args();
        assert!(!args.iter().any(|a| a == "--tor-bridges"));
    }

    #[test]
    fn v22_modes_emit() {
        let mut p = ConnectionProfile::default();
        p.protocol = Protocol::Mim;
        p.scan_mode = ScanMode::Verified;
        p.noize = Noize::Aggressive;
        p.peer = "1.2.3.4:443".into();
        p.exit_loc = "!IR,AZ,RU".into();
        p.api_fragment = true;
        p.ech = "auto".into();
        let args = p.as_args();
        for want in ["--mim", "--verified", "--noize", "aggressive", "--peer", "1.2.3.4:443",
            "--exit-loc", "!IR,AZ,RU", "--api-fragment", "--ech", "auto"] {
            assert!(args.iter().any(|a| a == want), "missing {want}: {args:?}");
        }
    }

    #[test]
    fn http_moves_aside_from_tor_exit() {
        // tor-inside with everything default: tor exit 1820, HTTP would be
        // 1820 too — HTTP moves to 1821, on both display and flag forms.
        let mut p = ConnectionProfile::default();
        p.extra_transport = ExtraTransport::Tor;
        assert_eq!(tor_exit_addr(&p).map(|s| s.port()), Some(1820));
        assert_eq!(http_proxy_socket_for(&p).to_string(), "127.0.0.1:1821");
        assert_eq!(flag_http_addr_for(&p), "127.0.0.1:1821");
        // Explicit tor bind elsewhere: no move.
        p.tor_bind = "127.0.0.1:1900".into();
        assert_eq!(http_proxy_socket_for(&p).to_string(), "127.0.0.1:1820");
        // No tor mode: no exits, no move.
        p.extra_transport = ExtraTransport::None;
        p.tor_bind = String::new();
        assert_eq!(tor_exit_addr(&p), None);
        assert_eq!(http_proxy_socket_for(&p).to_string(), "127.0.0.1:1820");
    }

    #[test]
    fn v22_tor_psiphon_extras_gated() {        let mut p = ConnectionProfile::default();
        p.extra_transport = ExtraTransport::TorOnly;
        p.tor_relays = "only".into();
        p.tor_bridge = "obfs4 1.2.3.4:443 FP cert=x iat-mode=0\n\nobfs4 5.6.7.8:443 FP2 cert=y".into();
        p.tor_bind = "127.0.0.1:1900".into();
        p.psiphon_region = "DE".into();
        let args = p.as_args();
        for want in ["--tor-relays", "only", "--tor-bridge", "--tor-bind", "127.0.0.1:1900"] {
            assert!(args.iter().any(|a| a == want), "missing {want}: {args:?}");
        }
        assert_eq!(args.iter().filter(|a| *a == "--tor-bridge").count(), 2);
        // Psiphon region stays off without a Psiphon mode.
        assert!(!args.iter().any(|a| a == "--psiphon-region"));
    }

    #[test]
    fn zero_trust_email_is_provided_as_an_environment_value() {        let p = ConnectionProfile {
            zero_trust_team: "acme".into(),
            access_email: "me@example.com".into(),
            ..Default::default()
        };
        assert_eq!(
            p.zero_trust_env(),
            Some(("AETHER_ACCESS_EMAIL", "me@example.com"))
        );
        assert!(!p.as_args().iter().any(|arg| arg.contains("me@example.com")));
    }
}

impl Default for ConnectionProfile {
    fn default() -> Self {
        // Mirrors Aether's own defaults.
        Self {
            protocol: Protocol::Auto,
            scan_mode: ScanMode::Balanced,
            ip_version: IpVersion::V4,
            quick_reconnect: true,
            masque_http2: false,
            noize: Noize::Firewall,
            bind_address: default_bind_address(),
            http_port: String::new(),
            dns: String::new(),
            zero_trust_team: String::new(),
            zero_trust_auth: ZeroTrustAuth::Email,
            access_email: String::new(),
            access_client_id: String::new(),
            access_client_secret: String::new(),
            access_token: String::new(),
            zero_trust_gateway: false,
            route_block: String::new(),
            route_direct: String::new(),
            routes_file: String::new(),
            peer: String::new(),
            wg_peer: String::new(),
            gool_peer: String::new(),
            wiw_outer: String::new(),
            wiw_inner: String::new(),
            mim_outer: String::new(),
            mim_inner: String::new(),
            exit_loc: String::new(),
            api_fragment: false,
            no_quic_v2: false,
            h2_peer: String::new(),
            no_data_check: false,
            no_profile_retry: false,
            keepalive: String::new(),
            perf: String::new(),
            ech: String::new(),
            ech_dns: String::new(),
            ech_domain: String::new(),
            tls_ciphers: String::new(),
            tls_groups: String::new(),
            disable_grease: false,
            masque_mtu: String::new(),
            netstack_rx: String::new(),
            netstack_tx: String::new(),
            route_sniff: true,
            tor_relays: String::new(),
            tor_bridge: String::new(),
            tor_bridge_file: String::new(),
            tor_bind: String::new(),
            psiphon_config: String::new(),
            psiphon_cdn_ips: String::new(),
            psiphon_cdn_sni: String::new(),
            psiphon_cdn_sets: String::new(),
            psiphon_server_entries: String::new(),
            psiphon_bind: String::new(),
            upstream: String::new(),
            extra_transport: ExtraTransport::None,
            tor_bridges: false,
            psiphon_region: String::new(),
            psiphon_mode: PsiphonMode::Auto,
        }
    }
}

const STORE_FILE: &str = "profile.json";
const STORE_KEY: &str = "last_successful_profile";

/// Loads the last profile that reached `Connected`, or the hardcoded default
/// on first run. Only ever written by `save()` at the moment a connection
/// actually succeeds (see aether/mod.rs) — never on a mere attempt, so a bad
/// guess can't poison future one-click connects.
pub fn load(app: &tauri::AppHandle) -> ConnectionProfile {
    use tauri_plugin_store::StoreExt;
    // v2.2 renamed the stealth scan mode to verified — migrate saved
    // profiles instead of dropping all their settings on parse failure.
    if let Ok(store) = app.store(STORE_FILE) {
        if let Some(mut value) = store.get(STORE_KEY) {
            migrate_stealth(&mut value);
            if let Ok(p) = serde_json::from_value(value) {
                return p;
            }
        }
    }
    ConnectionProfile::default()
}

/// Rewrites scan_mode "stealth" (removed in core v2.2) to "verified" in a
/// saved profile value, preserving every other setting.
fn migrate_stealth(value: &mut serde_json::Value) {
    if value.get("scan_mode").and_then(|v| v.as_str()) == Some("stealth") {
        if let Some(obj) = value.as_object_mut() {
            obj.insert("scan_mode".into(), serde_json::Value::from("verified"));
        }
    }
}

pub fn save(app: &tauri::AppHandle, profile: &ConnectionProfile) {
    use tauri_plugin_store::StoreExt;
    if let Ok(store) = app.store(STORE_FILE) {
        // A successful connection profile is useful to remember, but Access
        // credentials are not. Leave them in process memory only; the next
        // app launch will ask for them again rather than writing a JWT,
        // service secret or email address into profile.json.
        let mut persisted = profile.clone();
        persisted.access_email.clear();
        persisted.access_client_id.clear();
        persisted.access_client_secret.clear();
        persisted.access_token.clear();
        if let Ok(value) = serde_json::to_value(persisted) {
            store.set(STORE_KEY, value);
            let _ = store.save();
        }
    }
}
