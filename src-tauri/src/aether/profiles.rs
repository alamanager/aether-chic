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
}

impl Protocol {
    /// The literal menu choice Aether expects at its "Protocol:" prompt.
    pub fn as_menu_choice(&self) -> &'static str {
        match self {
            Protocol::Auto | Protocol::Masque => "1",
            Protocol::Wireguard => "2",
            Protocol::Gool => "3",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ScanMode {
    Turbo,
    Balanced,
    Thorough,
    Stealth,
    Ironclad,
}

impl ScanMode {
    pub fn as_menu_choice(&self) -> &'static str {
        match self {
            ScanMode::Turbo => "1",
            ScanMode::Balanced => "2",
            ScanMode::Thorough => "3",
            ScanMode::Stealth => "4",
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

/// Obfuscation profile for MASQUE connections. The profile shapes how much
/// junk/padding Aether injects to disguise the handshake from DPI.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MasqueNoize {
    Firewall,
    Gfw,
    Off,
}

impl MasqueNoize {
    pub fn as_flag(&self) -> &'static str {
        match self {
            MasqueNoize::Firewall => "firewall",
            MasqueNoize::Gfw => "gfw",
            MasqueNoize::Off => "off",
        }
    }
}

/// Obfuscation profile for WireGuard and gool connections.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WgNoize {
    Balanced,
    Aggressive,
    Light,
    Off,
}

impl WgNoize {
    pub fn as_flag(&self) -> &'static str {
        match self {
            WgNoize::Balanced => "balanced",
            WgNoize::Aggressive => "aggressive",
            WgNoize::Light => "light",
            WgNoize::Off => "off",
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
    /// Obfuscation profile for MASQUE (firewall/gfw/off). Passed as
    /// `--noize <value>`. Only sent when the active protocol is MASQUE-based.
    #[serde(default = "default_masque_noize")]
    pub masque_noize: MasqueNoize,
    /// Obfuscation profile for WireGuard/gool (balanced/aggressive/light/off).
    /// Only sent when the active protocol is WireGuard or gool.
    #[serde(default = "default_wg_noize")]
    pub wg_noize: WgNoize,
    /// Local SOCKS5 listen address (`--bind`). Aether defaults to
    /// 127.0.0.1:1819; users can change the port or bind to 0.0.0.0 for LAN.
    #[serde(default = "default_bind_address")]
    pub bind_address: String,
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

fn default_masque_noize() -> MasqueNoize {
    MasqueNoize::Firewall
}

fn default_wg_noize() -> WgNoize {
    WgNoize::Balanced
}

fn default_bind_address() -> String {
    "127.0.0.1:1819".into()
}

/// HTTP endpoint served by the core next to SOCKS5: same IP, port + 1.
/// Falls back to 127.0.0.1:1820 when the bind address doesn't parse.
pub fn http_proxy_addr(bind_address: &str) -> String {
    match bind_address.parse::<std::net::SocketAddr>() {
        Ok(mut socks) => {
            socks.set_port(socks.port().wrapping_add(1));
            socks.to_string()
        }
        Err(_) => "127.0.0.1:1820".into(),
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
        }
        args.push(match self.scan_mode {
            ScanMode::Turbo => "--turbo".into(),
            ScanMode::Balanced => "--balanced".into(),
            ScanMode::Thorough => "--thorough".into(),
            ScanMode::Stealth => "--stealth".into(),
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
        // Noize profile — pick the value matching the active protocol family.
        args.push("--noize".into());
        args.push(
            match self.protocol {
                Protocol::Auto | Protocol::Masque => self.masque_noize.as_flag(),
                Protocol::Wireguard | Protocol::Gool => self.wg_noize.as_flag(),
            }
            .into(),
        );
        // Only forward --bind when non-default and parseable.
        if self.bind_address != default_bind_address()
            && self.bind_address.parse::<std::net::SocketAddr>().is_ok()
        {
            args.push("--bind".into());
            args.push(self.bind_address.clone());
        }
        // Aether ≥1.6.0 serves a native HTTP CONNECT proxy next to SOCKS5 —
        // this replaced the GUI's old hand-rolled bridge. Always on, on its
        // own port (SOCKS port + 1) so the two stay separable in the UI.
        let http_addr = http_proxy_addr(&self.bind_address);
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
        assert_eq!(p.masque_noize, MasqueNoize::Firewall);
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
        assert_eq!(http_proxy_addr("127.0.0.1:1819"), "127.0.0.1:1820");
        assert_eq!(http_proxy_addr("0.0.0.0:1919"), "0.0.0.0:1920");
        assert_eq!(http_proxy_addr("garbage"), "127.0.0.1:1820");
        let p = ConnectionProfile::default();
        let args = p.as_args();
        let i = args.iter().position(|a| a == "--http-proxy").expect("missing --http-proxy");
        assert_eq!(args.get(i + 1).map(String::as_str), Some("127.0.0.1:1820"));
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
            masque_noize: MasqueNoize::Firewall,
            wg_noize: WgNoize::Balanced,
            bind_address: default_bind_address(),
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
    app.store(STORE_FILE)
        .ok()
        .and_then(|s| s.get(STORE_KEY))
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
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
