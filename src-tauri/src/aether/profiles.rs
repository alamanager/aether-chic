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
    /// Psiphon egress region (ISO alpha-2, e.g. "DE"). Empty = automatic.
    /// Only forwarded with a Psiphon mode; the core treats it as a hard
    /// filter, so a region with no current exit won't connect — retry Auto.
    #[serde(default)]
    pub psiphon_region: String,
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
        }
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

/// Fixed Tor/Psiphon exit listeners for chain/reverse modes (core
/// defaults — explicit fields were dropped to match the reference
/// implementation; only-modes serve on --bind itself).
pub const TOR_BIND: &str = "127.0.0.1:1820";
pub const PSIPHON_BIND: &str = "127.0.0.1:1821";

/// Effective Tor exit listener for inside/reverse modes. None otherwise
/// (tor-only serves on --bind itself).
pub fn tor_exit_addr(extra: &ExtraTransport) -> Option<std::net::SocketAddr> {
    if !matches!(
        extra,
        ExtraTransport::Tor | ExtraTransport::TorReverse
    ) {
        return None;
    }
    TOR_BIND.parse().ok()
}

/// Effective Psiphon exit listener for inside/reverse modes.
pub fn psi_exit_addr(extra: &ExtraTransport) -> Option<std::net::SocketAddr> {
    if !matches!(
        extra,
        ExtraTransport::Psiphon | ExtraTransport::PsiphonReverse
    ) {
        return None;
    }
    PSIPHON_BIND.parse().ok()
}

impl ConnectionProfile {
    /// Reverse modes carry only TCP, so the core refuses WireGuard/gool.
    /// Forced to MASQUE here (not just hidden in the UI) so a refused
    /// combo can never reach the core.
    pub fn effective_protocol(&self) -> Protocol {
        if self.is_reverse()
            && matches!(
                self.protocol,
                Protocol::Wireguard | Protocol::Gool | Protocol::GoolClassic
            )
        {
            Protocol::Masque
        } else {
            self.protocol.clone()
        }
    }

    pub fn is_tor(&self) -> bool {
        matches!(
            self.extra_transport,
            ExtraTransport::Tor | ExtraTransport::TorReverse | ExtraTransport::TorOnly
        )
    }

    pub fn is_psiphon(&self) -> bool {
        matches!(
            self.extra_transport,
            ExtraTransport::Psiphon | ExtraTransport::PsiphonReverse | ExtraTransport::PsiphonOnly
        )
    }

    pub fn is_reverse(&self) -> bool {
        matches!(
            self.extra_transport,
            ExtraTransport::TorReverse | ExtraTransport::PsiphonReverse
        )
    }

    /// Extra seconds on top of the scan budget: Tor may try plainly (75s)
    /// then walk bridges; Psiphon waits up to 180s to tunnel.
    pub fn extra_wait_secs(&self) -> u64 {
        if self.is_tor() {
            480
        } else if self.is_psiphon() {
            200
        } else {
            0
        }
    }

    /// Extra proxy address for chain/reverse modes (Tor 1820 / Psiphon
    /// 1821 by default, explicit field wins), loopback-mapped for display.
    /// None otherwise — only-modes serve on --bind itself.
    pub fn tor_exit_addr(&self) -> Option<std::net::SocketAddr> {
        tor_exit_addr(&self.extra_transport)
    }

    pub fn psi_exit_addr(&self) -> Option<std::net::SocketAddr> {
        psi_exit_addr(&self.extra_transport)
    }

    /// Ports that must answer before the GUI reports connected: the SOCKS
    /// bind plus the Tor/Psiphon exit listener in chain/reverse modes.
    pub fn ready_addrs(&self) -> Vec<std::net::SocketAddr> {
        let mut out = vec![crate::aether::status::parse_bind_address(&self.bind_address)];
        out.extend(self.tor_exit_addr());
        out.extend(self.psi_exit_addr());
        out
    }

    /// Ports that must be free before launching: readiness plus the HTTP
    /// frontend (a stale occupant there would leave SOCKS working while
    /// the system proxy talks to a dead port).
    pub fn listen_addrs(&self) -> Vec<std::net::SocketAddr> {
        let mut out = self.ready_addrs();
        out.push(self.frontend_http_addr());
        out
    }

    /// The SOCKS5 address users should point apps at. In chain modes that
    /// is the Tor/Psiphon exit (--bind keeps the plain WARP exit).
    pub fn primary_addr(&self) -> String {
        if self.extra_transport == ExtraTransport::Tor
            || self.extra_transport == ExtraTransport::TorReverse
        {
            if let Some(a) = self.tor_exit_addr() {
                return a.to_string();
            }
        }
        if self.extra_transport == ExtraTransport::Psiphon
            || self.extra_transport == ExtraTransport::PsiphonReverse
        {
            if let Some(a) = self.psi_exit_addr() {
                return a.to_string();
            }
        }
        crate::aether::status::client_addr(
            &self.bind_address.parse().unwrap_or_else(|_| {
                "127.0.0.1:1819".parse().expect("literal parses")
            }),
        )
        .to_string()
    }

    /// Which flag serves the HTTP frontend in this mode: each Tor/Psiphon
    /// chain has its own HTTP flag; plain and reverse modes serve HTTP
    /// from the main proxy.
    pub fn http_front_flag(&self) -> &'static str {
        match self.extra_transport {
            ExtraTransport::Psiphon
            | ExtraTransport::PsiphonReverse
            | ExtraTransport::PsiphonOnly => "--psiphon-http",
            ExtraTransport::Tor | ExtraTransport::TorReverse | ExtraTransport::TorOnly => {
                "--tor-http"
            }
            _ => "--http-proxy",
        }
    }

    /// The HTTP frontend address for this mode, loopback-mapped: the
    /// custom port (unless colliding), else SOCKS+1 — except Tor
    /// inside/reverse, where the Tor exit owns 1820 and HTTP moves to 1822.
    pub fn frontend_http_addr(&self) -> std::net::SocketAddr {
        let mut http = super::status::client_addr(
            &self
                .bind_address
                .parse()
                .unwrap_or_else(|_| "127.0.0.1:1819".parse().expect("literal parses")),
        );
        http.set_port(self.frontend_http_port());
        http
    }

    /// The --http-proxy/--tor-http/--psiphon-http flag value: the resolved
    /// port on the bind IP (so LAN sharing covers HTTP too).
    pub fn flag_http_addr(&self) -> String {
        let port = self.frontend_http_port();
        match self.bind_address.parse::<std::net::SocketAddr>() {
            Ok(socks) => std::net::SocketAddr::new(socks.ip(), port).to_string(),
            Err(_) => format!("127.0.0.1:{port}"),
        }
    }

    /// Resolved HTTP port: custom wins unless invalid or colliding with
    /// SOCKS; else SOCKS+1 — except Tor inside/reverse, where the Tor exit
    /// owns 1820 and HTTP moves to 1822.
    fn frontend_http_port(&self) -> u16 {
        let socks: std::net::SocketAddr = self
            .bind_address
            .parse()
            .unwrap_or_else(|_| "127.0.0.1:1819".parse().expect("literal parses"));
        let tor = matches!(
            self.extra_transport,
            ExtraTransport::Tor | ExtraTransport::TorReverse
        );
        match self.http_port.trim().parse::<u16>() {
            Ok(p) if p >= 1 && p != socks.port() => p,
            _ => {
                if tor {
                    1822
                } else {
                    let q = socks.port().wrapping_add(1);
                    if q == 0 {
                        1820
                    } else {
                        q
                    }
                }
            }
        }
    }

    /// CLI flags for Aether ≥1.1.1 — the whole profile is passed up front so
    /// the interactive prompts never appear (the PTY prompt-answering in
    /// pty.rs stays as a fallback). One of the two quick-reconnect flags is
    /// ALWAYS passed: without either, 1.1.1 asks its own interactive
    /// "reconnect with last gateway?" question, which the GUI must never
    /// leave unanswered.
    pub fn as_args(&self) -> Vec<String> {
        let mut args = Vec::with_capacity(24);
        // Reverse modes carry only TCP, so the core refuses WireGuard/gool:
        // fall back to MASQUE rather than sending a refused combo.
        match self.effective_protocol() {
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
        // Each Tor/Psiphon chain has its own HTTP flag; plain and reverse
        // modes serve HTTP from the main proxy. The HTTP frontend is a
        // convenience, never anything to wait on — but it must be served
        // for the system proxy and the UI to use it.
        let http_addr = self.flag_http_addr();
        args.push(self.http_front_flag().into());
        args.push(http_addr);
        if !self.upstream.trim().is_empty() {
            args.push("--upstream".into());
            args.push(self.upstream.trim().into());
        }
        if let Some(flag) = self.extra_transport.as_flag() {
            args.push(flag.into());
        }
        // Psiphon exit country, gated on a Psiphon mode and validated to a
        // 2-letter code (the core treats it as a hard filter).
        if matches!(
            self.extra_transport,
            ExtraTransport::Psiphon | ExtraTransport::PsiphonReverse | ExtraTransport::PsiphonOnly
        ) {
            let region = self.psiphon_region.trim().to_ascii_uppercase();
            if region.len() == 2 && region.bytes().all(|b| b.is_ascii_alphabetic()) {
                args.push("--psiphon-region".into());
                args.push(region);
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
        assert_eq!(q.flag_http_addr(), "0.0.0.0:1820");
        q.bind_address = "0.0.0.0:1919".into();
        q.http_port = "18080".into();
        assert_eq!(q.flag_http_addr(), "0.0.0.0:18080");
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
    fn psiphon_region_gated_and_validated() {
        let mut p = ConnectionProfile::default();
        p.extra_transport = ExtraTransport::PsiphonOnly;
        p.psiphon_region = "de".into();
        let args = p.as_args();
        let i = args.iter().position(|a| a == "--psiphon-region").expect("missing region");
        assert_eq!(args.get(i + 1).map(String::as_str), Some("DE"));
        // Junk is not forwarded.
        p.psiphon_region = "xyz1".into();
        let args = p.as_args();
        assert!(!args.iter().any(|a| a == "--psiphon-region"));
        // Without a Psiphon mode nothing is forwarded.
        p.psiphon_region = "DE".into();
        p.extra_transport = ExtraTransport::TorOnly;
        let args = p.as_args();
        assert!(!args.iter().any(|a| a == "--psiphon-region"));
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
        // 1820 too — HTTP moves to 1822, on both display and flag forms.
        let mut p = ConnectionProfile::default();
        p.extra_transport = ExtraTransport::Tor;
        assert_eq!(p.tor_exit_addr().map(|s| s.port()), Some(1820));
        assert_eq!(p.frontend_http_addr().to_string(), "127.0.0.1:1822");
        assert_eq!(p.flag_http_addr(), "127.0.0.1:1822");
        assert_eq!(p.http_front_flag(), "--tor-http");
        // psiphon-inside: exit 1821, HTTP stays on 1820 via --psiphon-http.
        p.extra_transport = ExtraTransport::Psiphon;
        assert_eq!(p.psi_exit_addr().map(|s| s.port()), Some(1821));
        assert_eq!(p.frontend_http_addr().to_string(), "127.0.0.1:1820");
        assert_eq!(p.http_front_flag(), "--psiphon-http");
        assert_eq!(p.primary_addr(), "127.0.0.1:1821");
        // Warp default: plain --http-proxy on 1820, primary is the bind.
        p.extra_transport = ExtraTransport::None;
        assert_eq!(p.tor_exit_addr(), None);
        assert_eq!(p.frontend_http_addr().to_string(), "127.0.0.1:1820");
        assert_eq!(p.http_front_flag(), "--http-proxy");
        assert_eq!(p.primary_addr(), "127.0.0.1:1819");
        // Reverse forces MASQUE even with WireGuard selected.
        p.extra_transport = ExtraTransport::TorReverse;
        p.protocol = Protocol::Wireguard;
        assert_eq!(p.effective_protocol(), Protocol::Masque);
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
            upstream: String::new(),
            extra_transport: ExtraTransport::None,
            psiphon_region: String::new(),
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
