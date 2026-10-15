//! Share-link parsers → engine outbound JSON (sing-box or Xray).
//!
//! Supported: vless / vmess / trojan / ss (no plugins) / hysteria2 / tuic /
//! wireguard / socks / http(s). Reality always routes to Xray (best
//! REALITY stack); everything else rides sing-box. Recognized but
//! engine-less: amnezia `vpn://`, `ssh://`, hysteria v1 — stored with a
//! clear reason instead of a fake mapping that would silently misconnect.
use base64::Engine as _;
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    SingBox,
    Xray,
    Ssh,
    Ikev2,
}

#[derive(Debug, Clone)]
pub struct ParsedLink {
    pub name: String,
    pub engine: Engine,
    /// Engine outbound object (sing-box `type` / xray `protocol` shape).
    pub outbound: Value,
    pub server_host: String,
    pub server_port: u16,
}

#[derive(Debug, Clone)]
pub enum LinkError {
    Unsupported(String),
    Malformed(String),
}

pub fn b64_decode(s: &str) -> Result<Vec<u8>, String> {
    // Accept standard and URL-safe alphabets, with or without padding.
    let mut norm: String = s
        .trim()
        .chars()
        .map(|c| match c {
            '-' => '+',
            '_' => '/',
            c => c,
        })
        .collect();
    while norm.len() % 4 != 0 {
        norm.push('=');
    }
    base64::engine::general_purpose::STANDARD
        .decode(&norm)
        .map_err(|e| format!("base64: {e}"))
}

/// Minimal %XX decoder (NOT form-decoding: a literal '+' stays '+',
// otherwise passwords get corrupted).
fn pct_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.as_bytes().iter().peekable();
    while let Some(&b) = it.next() {
        if b == b'%' {
            let hi = it.next().copied().unwrap_or(b'0');
            let lo = it.next().copied().unwrap_or(b'0');
            let hex = |c: u8| (c as char).to_digit(16).unwrap_or(0) as u8;
            out.push((hex(hi) * 16 + hex(lo)) as char);
        } else if b == b'+' {
            // Literal plus: NOT a space (form-encoding only). Passwords
            // routinely contain '+' typed as-is.
            out.push('+');
        } else {
            out.push(b as char);
        }
    }
    out
}

fn query_map(url: &url::Url) -> HashMap<String, String> {
    url.query_pairs().map(|(k, v)| (k.into_owned(), v.into_owned())).collect()
}

fn tls_block(q: &HashMap<String, String>, host: &str) -> Option<Value> {
    let sni = q.get("sni").map(|s| s.as_str()).unwrap_or(host);
    if sni.is_empty() {
        return None;
    }
    let mut tls = serde_json::Map::new();
    tls.insert("enabled".into(), Value::Bool(true));
    tls.insert("server_name".into(), Value::String(sni.to_string()));
    if let Some(fp) = q.get("fp") {
        if !fp.is_empty() && fp != "none" {
            tls.insert(
                "utls".into(),
                json!({"enabled": true, "fingerprint": fp}),
            );
        }
    }
    if let Some(alpn) = q.get("alpn") {
        let v: Vec<Value> = alpn.split(',').filter(|s| !s.is_empty()).map(|s| json!(s)).collect();
        if !v.is_empty() {
            tls.insert("alpn".into(), Value::Array(v));
        }
    }
    if q.get("insecure").map(|s| s == "1" || s.eq_ignore_ascii_case("true")).unwrap_or(false) {
        tls.insert("insecure".into(), Value::Bool(true));
    }
    Some(Value::Object(tls))
}

/// sing-box transport (ws / grpc / httpupgrade); tcp+none → None.
fn sb_transport(q: &HashMap<String, String>) -> Option<Value> {
    match q.get("type").map(|s| s.as_str()).unwrap_or("tcp") {
        "ws" => {
            let mut m = serde_json::Map::new();
            m.insert("type".into(), json!("ws"));
            m.insert("path".into(), json!(q.get("path").map(|s| s.as_str()).unwrap_or("/")));
            if let Some(h) = q.get("host") {
                if !h.is_empty() {
                    m.insert("headers".into(), json!({"Host": h}));
                }
            }
            Some(Value::Object(m))
        }
        "grpc" => {
            let mut m = serde_json::Map::new();
            m.insert("type".into(), json!("grpc"));
            m.insert(
                "service_name".into(),
                json!(q.get("serviceName").or(q.get("service_name")).map(|s| s.as_str()).unwrap_or("")),
            );
            Some(Value::Object(m))
        }
        "httpupgrade" => {
            let mut m = serde_json::Map::new();
            m.insert("type".into(), json!("httpupgrade"));
            m.insert("path".into(), json!(q.get("path").map(|s| s.as_str()).unwrap_or("/")));
            if let Some(h) = q.get("host") {
                if !h.is_empty() {
                    m.insert("host".into(), json!(h));
                }
            }
            Some(Value::Object(m))
        }
        _ => None,
    }
}

fn split_name(raw: &str) -> (String, String) {
    match raw.find('#') {
        Some(i) => (raw[..i].to_string(), pct_decode(&raw[i + 1..])),
        None => (raw.to_string(), String::new()),
    }
}

pub fn parse_link(raw: &str) -> Result<ParsedLink, LinkError> {
    let raw = raw.trim();
    let scheme = raw.split("://").next().unwrap_or("").to_lowercase();
    match scheme.as_str() {
        "vless" => parse_vless(raw),
        "vmess" => parse_vmess(raw),
        "trojan" => parse_trojan(raw),
        "ss" | "shadowsocks" => parse_ss(raw),
        "hy2" | "hysteria2" => parse_hy2(raw),
        "hysteria" => Err(LinkError::Unsupported("hysteria v1 needs its own engine".into())),
        "tuic" => parse_tuic(raw),
        "wireguard" | "wg" => parse_wg(raw),
        "socks" | "socks5" => parse_socks(raw, false),
        "http" | "https" => parse_socks(raw, true),
        "vpn" => Err(LinkError::Unsupported(
            "Amnezia vpn:// needs the Amnezia engine (coming)".into(),
        )),
        "ssh" => parse_ssh(raw),
        "ikev2" => parse_ikev2(raw),
        "" => Err(LinkError::Malformed("empty link".into())),
        other => Err(LinkError::Unsupported(format!("unknown scheme: {other}"))),
    }
}

fn url_of(main: &str, what: &str) -> Result<url::Url, LinkError> {
    url::Url::parse(main).map_err(|e| LinkError::Malformed(format!("{what}: {e}")))
}

fn req_port(u: &url::Url) -> Result<u16, LinkError> {
    u.port().ok_or_else(|| LinkError::Malformed("missing port".into()))
}

fn parse_vless(raw: &str) -> Result<ParsedLink, LinkError> {
    let (main, name) = split_name(raw);
    let u = url_of(&main, "vless")?;
    let uuid = u.username().to_string();
    if uuid.is_empty() {
        return Err(LinkError::Malformed("missing uuid".into()));
    }
    let host = u.host_str().unwrap_or("").to_string();
    let port = req_port(&u)?;
    let q = query_map(&u);
    let sec = q.get("security").map(|s| s.as_str()).unwrap_or("none");
    if sec == "reality" {
        return Ok(xray_reality(&name, &uuid, &host, port, &q));
    }
    let mut ob = serde_json::Map::new();
    ob.insert("type".into(), json!("vless"));
    ob.insert("tag".into(), json!("proxy"));
    ob.insert("server".into(), json!(host));
    ob.insert("server_port".into(), json!(port));
    ob.insert("uuid".into(), json!(uuid));
    if let Some(f) = q.get("flow") {
        if !f.is_empty() {
            ob.insert("flow".into(), json!(f));
        }
    }
    if sec == "tls" {
        if let Some(t) = tls_block(&q, &host) {
            ob.insert("tls".into(), t);
        }
    }
    if let Some(t) = sb_transport(&q) {
        ob.insert("transport".into(), t);
    }
    Ok(ParsedLink {
        name: name_or(&name, &host),
        engine: Engine::SingBox,
        outbound: Value::Object(ob),
        server_host: host,
        server_port: port,
    })
}

fn xray_reality(name: &str, uuid: &str, host: &str, port: u16, q: &HashMap<String, String>) -> ParsedLink {
    let net = q.get("type").map(|s| s.as_str()).unwrap_or("tcp");
    let mut stream: serde_json::Map<String, Value> = serde_json::Map::new();
    stream.insert("network".into(), json!(net));
    stream.insert("security".into(), json!("reality"));
    let mut reality = serde_json::Map::new();
    reality.insert("serverName".into(), json!(q.get("sni").map(|s| s.as_str()).unwrap_or(host)));
    reality.insert("fingerprint".into(), json!(q.get("fp").map(|s| s.as_str()).unwrap_or("chrome")));
    reality.insert("publicKey".into(), json!(q.get("pbk").map(|s| s.as_str()).unwrap_or("")));
    reality.insert("shortId".into(), json!(q.get("sid").map(|s| s.as_str()).unwrap_or("")));
    if let Some(spx) = q.get("spiderX") {
        if !spx.is_empty() {
            reality.insert("spiderX".into(), json!(spx));
        }
    }
    stream.insert("realitySettings".into(), Value::Object(reality));
    match net {
        "ws" => {
            let mut ws = serde_json::Map::new();
            ws.insert("path".into(), json!(q.get("path").map(|s| s.as_str()).unwrap_or("/")));
            if let Some(h) = q.get("host") {
                if !h.is_empty() {
                    ws.insert("headers".into(), json!({"Host": h}));
                }
            }
            stream.insert("wsSettings".into(), Value::Object(ws));
        }
        "grpc" => {
            stream.insert(
                "grpcSettings".into(),
                json!({"serviceName": q.get("serviceName").or(q.get("service_name")).map(|s| s.as_str()).unwrap_or("")}),
            );
        }
        "httpupgrade" => {
            let mut m = serde_json::Map::new();
            m.insert("path".into(), json!(q.get("path").map(|s| s.as_str()).unwrap_or("/")));
            if let Some(h) = q.get("host") {
                if !h.is_empty() {
                    m.insert("host".into(), json!(h));
                }
            }
            stream.insert("httpupgradeSettings".into(), Value::Object(m));
        }
        "xhttp" => {
            // Best-effort shape for Xray 25+: wrong keys fail loudly in the
            // log, never silently � see docs before relying on xhttp links.
            let mut m = serde_json::Map::new();
            m.insert("mode".into(), json!("auto"));
            m.insert("path".into(), json!(q.get("path").map(|s| s.as_str()).unwrap_or("/")));
            if let Some(h) = q.get("host") {
                if !h.is_empty() {
                    m.insert("host".into(), json!(h));
                }
            }
            stream.insert("xhttpSettings".into(), Value::Object(m));
        }
        _ => {}
    }
    let flow = q.get("flow").cloned().unwrap_or_default();
    let outbound = json!({
        "protocol": "vless",
        "settings": {"vnext": [{"address": host, "port": port,
            "users": [{"id": uuid, "flow": flow, "encryption": "none"}]}]},
        "streamSettings": stream,
        "tag": "proxy",
    });
    ParsedLink {
        name: name_or(name, host),
        engine: Engine::Xray,
        outbound,
        server_host: host.to_string(),
        server_port: port,
    }
}

fn name_or(name: &str, host: &str) -> String {
    if name.trim().is_empty() {
        host.to_string()
    } else {
        name.to_string()
    }
}

fn parse_vmess(raw: &str) -> Result<ParsedLink, LinkError> {
    let (main, _) = split_name(raw);
    // Strip exactly one scheme prefix (case-insensitive) — never chew
    // into the base64 body, which may itself start with v/V.
    let body = if main.to_lowercase().starts_with("vmess://") {
        main.get("vmess://".len()..).unwrap_or("")
    } else {
        main.as_str()
    };
    let decoded = b64_decode(body).map_err(LinkError::Malformed)?;
    let v: Value =
        serde_json::from_slice(&decoded).map_err(|e| LinkError::Malformed(format!("vmess json: {e}")))?;
    let g = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let host = g("add");
    let port: u16 = g("port").parse().map_err(|_| LinkError::Malformed("bad vmess port".into()))?;
    if host.is_empty() || g("id").is_empty() {
        return Err(LinkError::Malformed("missing vmess host/id".into()));
    }
    let mut ob = serde_json::Map::new();
    ob.insert("type".into(), json!("vmess"));
    ob.insert("tag".into(), json!("proxy"));
    ob.insert("server".into(), json!(host));
    ob.insert("server_port".into(), json!(port));
    ob.insert("uuid".into(), json!(g("id")));
    ob.insert("alter_id".into(), json!(g("aid").parse::<u16>().unwrap_or(0)));
    let scy = g("scy");
    ob.insert("security".into(), json!(if scy.is_empty() { "auto" } else { scy.as_str() }));
    if g("tls") == "tls" {
        let mut q = HashMap::new();
        if !g("sni").is_empty() {
            q.insert("sni".into(), g("sni"));
        }
        if !g("fp").is_empty() {
            q.insert("fp".into(), g("fp"));
        }
        if !g("alpn").is_empty() {
            q.insert("alpn".into(), g("alpn"));
        }
        if let Some(t) = tls_block(&q, &host) {
            ob.insert("tls".into(), t);
        }
    }
    match g("net").as_str() {
        "ws" => {
            let mut m = serde_json::Map::new();
            m.insert("type".into(), json!("ws"));
            let path = g("path");
            m.insert("path".into(), json!(if path.is_empty() { "/" } else { path.as_str() }));
            if !g("host").is_empty() {
                m.insert("headers".into(), json!({"Host": g("host")}));
            }
            ob.insert("transport".into(), Value::Object(m));
        }
        "grpc" => {
            ob.insert("transport".into(), json!({"type": "grpc", "service_name": g("serviceName")}));
        }
        "h2" => {
            let mut m = serde_json::Map::new();
            m.insert("type".into(), json!("http"));
            m.insert("host".into(), json!(if g("host").is_empty() { vec![host.clone()] } else { vec![g("host")] }));
            let path = g("path");
            m.insert("path".into(), json!(if path.is_empty() { "/" } else { path.as_str() }));
            ob.insert("transport".into(), Value::Object(m));
        }
        _ => {}
    }
    Ok(ParsedLink {
        name: name_or(&g("ps"), &host),
        engine: Engine::SingBox,
        outbound: Value::Object(ob),
        server_host: host,
        server_port: port,
    })
}

fn parse_trojan(raw: &str) -> Result<ParsedLink, LinkError> {
    let (main, name) = split_name(raw);
    let u = url_of(&main, "trojan")?;
    let pass = pct_decode(u.username());
    if pass.is_empty() {
        return Err(LinkError::Malformed("missing trojan password".into()));
    }
    let host = u.host_str().unwrap_or("").to_string();
    let port = req_port(&u)?;
    let q = query_map(&u);
    let mut ob = serde_json::Map::new();
    ob.insert("type".into(), json!("trojan"));
    ob.insert("tag".into(), json!("proxy"));
    ob.insert("server".into(), json!(host));
    ob.insert("server_port".into(), json!(port));
    ob.insert("password".into(), json!(pass));
    if let Some(t) = tls_block(&q, &host) {
        ob.insert("tls".into(), t);
    }
    if let Some(t) = sb_transport(&q) {
        ob.insert("transport".into(), t);
    }
    Ok(ParsedLink {
        name: name_or(&name, &host),
        engine: Engine::SingBox,
        outbound: Value::Object(ob),
        server_host: host,
        server_port: port,
    })
}

fn parse_ss(raw: &str) -> Result<ParsedLink, LinkError> {
    let (main, name) = split_name(raw);
    let after = main.splitn(2, "://").nth(1).unwrap_or("").to_string();
    // SIP002 (cred@host) or legacy whole-blob.
    let (method, password, hostport, query) = if after.contains('@') {
        let at = after.rfind('@').unwrap();
        let cred = b64_decode(&after[..at]).map_err(|e| LinkError::Malformed(format!("ss cred: {e}")))?;
        let cred = String::from_utf8(cred).map_err(|_| LinkError::Malformed("ss cred utf8".into()))?;
        let (m, p) = cred.split_once(':').ok_or_else(|| LinkError::Malformed("ss method:password".into()))?;
        let (hp, qq) = match after[at + 1..].find('?') {
            Some(i) => (after[at + 1..at + 1 + i].to_string(), after[at + 1 + i + 1..].to_string()),
            None => (after[at + 1..].to_string(), String::new()),
        };
        (m.to_string(), p.to_string(), hp, qq)
    } else {
        let decoded = b64_decode(&after).map_err(|e| LinkError::Malformed(format!("ss blob: {e}")))?;
        let s = String::from_utf8(decoded).map_err(|_| LinkError::Malformed("ss blob utf8".into()))?;
        let (credhost, qq) = match s.find('?') {
            Some(i) => (s[..i].to_string(), s[i + 1..].to_string()),
            None => (s, String::new()),
        };
        let at = credhost.rfind('@').ok_or_else(|| LinkError::Malformed("ss blob shape".into()))?;
        let (m, p) = credhost[..at].split_once(':').ok_or_else(|| LinkError::Malformed("ss method:password".into()))?;
        (m.to_string(), p.to_string(), credhost[at + 1..].to_string(), qq)
    };
    if query.contains("plugin=") {
        return Err(LinkError::Unsupported("ss plugins (obfs) need their own engine".into()));
    }
    let hp = hostport.split('?').next().unwrap_or("");
    let (host, port) = match hp.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse::<u16>().map_err(|_| LinkError::Malformed("ss port".into()))?),
        None => return Err(LinkError::Malformed("ss host:port".into())),
    };
    Ok(ParsedLink {
        name: name_or(&name, &host),
        engine: Engine::SingBox,
        outbound: json!({"type": "shadowsocks", "tag": "proxy",
            "server": host, "server_port": port, "method": method, "password": password}),
        server_host: host,
        server_port: port,
    })
}

fn parse_hy2(raw: &str) -> Result<ParsedLink, LinkError> {
    let (main, name) = split_name(raw);
    let u = url_of(&main, "hysteria2")?;
    let pass = pct_decode(u.username());
    if pass.is_empty() {
        return Err(LinkError::Malformed("missing hy2 password".into()));
    }
    let host = u.host_str().unwrap_or("").to_string();
    let port = req_port(&u)?;
    let q = query_map(&u);
    let mut ob = serde_json::Map::new();
    ob.insert("type".into(), json!("hysteria2"));
    ob.insert("tag".into(), json!("proxy"));
    ob.insert("server".into(), json!(host));
    ob.insert("server_port".into(), json!(port));
    ob.insert("password".into(), json!(pass));
    if let Some(o) = q.get("obfs") {
        if !o.is_empty() && o != "none" {
            ob.insert(
                "obfs".into(),
                json!({"type": o, "password": q.get("obfs-password").map(|s| s.as_str()).unwrap_or("")}),
            );
        }
    }
    // hysteria2 is always TLS; force server_name even without ?sni=.
    let mut qq = q.clone();
    if !qq.contains_key("sni") {
        qq.insert("sni".into(), host.clone());
    }
    if let Some(t) = tls_block(&qq, &host) {
        ob.insert("tls".into(), t);
    }
    Ok(ParsedLink {
        name: name_or(&name, &host),
        engine: Engine::SingBox,
        outbound: Value::Object(ob),
        server_host: host,
        server_port: port,
    })
}

fn parse_tuic(raw: &str) -> Result<ParsedLink, LinkError> {
    let (main, name) = split_name(raw);
    let u = url_of(&main, "tuic")?;
    let uuid = pct_decode(u.username());
    let pass = u.password().unwrap_or("").to_string();
    if uuid.is_empty() {
        return Err(LinkError::Malformed("missing tuic uuid".into()));
    }
    let host = u.host_str().unwrap_or("").to_string();
    let port = req_port(&u)?;
    let q = query_map(&u);
    let mut ob = serde_json::Map::new();
    ob.insert("type".into(), json!("tuic"));
    ob.insert("tag".into(), json!("proxy"));
    ob.insert("server".into(), json!(host));
    ob.insert("server_port".into(), json!(port));
    ob.insert("uuid".into(), json!(uuid));
    ob.insert("password".into(), json!(pass));
    ob.insert(
        "congestion_control".into(),
        json!(q.get("congestion_control").map(|s| s.as_str()).unwrap_or("cubic")),
    );
    if let Some(m) = q.get("udp_relay_mode") {
        if !m.is_empty() {
            ob.insert("udp_relay_mode".into(), json!(m));
        }
    }
    if let Some(t) = tls_block(&q, &host) {
        ob.insert("tls".into(), t);
    }
    Ok(ParsedLink {
        name: name_or(&name, &host),
        engine: Engine::SingBox,
        outbound: Value::Object(ob),
        server_host: host,
        server_port: port,
    })
}

fn parse_wg(raw: &str) -> Result<ParsedLink, LinkError> {
    let (main, name) = split_name(raw);
    let u = url_of(&main, "wireguard")?;
    let privkey = pct_decode(u.username());
    if privkey.is_empty() {
        return Err(LinkError::Malformed("missing wireguard private key".into()));
    }
    let host = u.host_str().unwrap_or("").to_string();
    let port = req_port(&u)?;
    let q = query_map(&u);
    let pubkey = q.get("publickey").or(q.get("public_key")).cloned().unwrap_or_default();
    if pubkey.is_empty() {
        return Err(LinkError::Malformed("missing wireguard peer public key".into()));
    }
    let mut ob = serde_json::Map::new();
    ob.insert("type".into(), json!("wireguard"));
    ob.insert("tag".into(), json!("proxy"));
    ob.insert("server".into(), json!(host));
    ob.insert("server_port".into(), json!(port));
    ob.insert("private_key".into(), json!(privkey));
    ob.insert("peer_public_key".into(), json!(pubkey));
    ob.insert("local_address".into(), json!(["172.19.0.2/30"]));
    let allowed: Vec<Value> = q
        .get("allowedips")
        .or(q.get("allowed_ips"))
        .map(|s| s.split(',').filter(|x| !x.is_empty()).map(|x| json!(x)).collect())
        .unwrap_or_else(|| vec![json!("0.0.0.0/0")]);
    ob.insert("allowed_ips".into(), Value::Array(allowed));
    if let Some(psk) = q.get("presharedkey").or(q.get("pre_shared_key")) {
        if !psk.is_empty() {
            ob.insert("pre_shared_key".into(), json!(psk));
        }
    }
    if let Some(ka) = q.get("keepalive").or(q.get("persistent_keepalive")) {
        if let Ok(n) = ka.parse::<u64>() {
            ob.insert("persistent_keepalive_interval".into(), json!(n));
        }
    }
    if let Some(mtu) = q.get("mtu") {
        if let Ok(n) = mtu.parse::<u64>() {
            ob.insert("mtu".into(), json!(n));
        }
    }
    Ok(ParsedLink {
        name: name_or(&name, &host),
        engine: Engine::SingBox,
        outbound: Value::Object(ob),
        server_host: host,
        server_port: port,
    })
}

fn parse_socks(raw: &str, is_http: bool) -> Result<ParsedLink, LinkError> {
    let (main, name) = split_name(raw);
    let u = url_of(&main, "proxy")?;
    let host = u.host_str().unwrap_or("").to_string();
    let port = req_port(&u)?;
    let user = pct_decode(u.username());
    let pass = u.password().map(pct_decode).unwrap_or_default();
    let mut ob = serde_json::Map::new();
    ob.insert("type".into(), json!(if is_http { "http" } else { "socks" }));
    ob.insert("tag".into(), json!("proxy"));
    ob.insert("server".into(), json!(host));
    ob.insert("server_port".into(), json!(port));
    if !user.is_empty() {
        ob.insert("username".into(), json!(user));
        ob.insert("password".into(), json!(pass));
    }
    if is_http && u.scheme() == "https" {
        ob.insert("tls".into(), json!({"enabled": true, "server_name": host}));
    }
    Ok(ParsedLink {
        name: name_or(&name, &host),
        engine: Engine::SingBox,
        outbound: Value::Object(ob),
        server_host: host,
        server_port: port,
    })
}

/// Our own conventions (documented in the SSH/IKEv2 forms):
/// ssh://user@host:port?key=BASE64 (key required — no passwords headless),
/// ikev2://user:pass@server#name.
fn parse_ssh(raw: &str) -> Result<ParsedLink, LinkError> {
    let (main, name) = split_name(raw);
    let u = url_of(&main, "ssh")?;
    let user = pct_decode(u.username());
    let host = u.host_str().unwrap_or("").to_string();
    let port = u.port().unwrap_or(22);
    if user.is_empty() || host.is_empty() {
        return Err(LinkError::Malformed("ssh needs user@host".into()));
    }
    let q: HashMap<String, String> =
        u.query_pairs().map(|(k, v)| (k.into_owned(), v.into_owned())).collect();
    // Key travels inside the link only as base64 (?key=...); the form
    // pastes raw key text instead. Either way it lands in one field.
    let key = q.get("key").cloned().unwrap_or_default();
    let params = serde_json::json!({
        "user": user, "host": host, "port": port, "key": key,
    });
    Ok(ParsedLink {
        name: name_or(&name, &host),
        engine: Engine::Ssh,
        outbound: params,
        server_host: host,
        server_port: port,
    })
}

fn parse_ikev2(raw: &str) -> Result<ParsedLink, LinkError> {
    let (main, name) = split_name(raw);
    let u = url_of(&main, "ikev2")?;
    let user = pct_decode(u.username());
    let pass = u.password().map(pct_decode).unwrap_or_default();
    let server = u.host_str().unwrap_or("").to_string();
    if user.is_empty() || server.is_empty() || pass.is_empty() {
        return Err(LinkError::Malformed("ikev2 needs user:pass@server".into()));
    }
    let params = serde_json::json!({
        "user": user, "pass": pass, "server": server,
    });
    Ok(ParsedLink {
        name: name_or(&name, &server),
        engine: Engine::Ikev2,
        outbound: params,
        server_host: server,
        server_port: 500,
    })
}

/// Top-level sing-box runtime config wrapping one outbound.
pub fn singbox_config(outbound: &Value) -> Value {
    let mut proxy = outbound.clone();
    if let Some(o) = proxy.as_object_mut() {
        o.insert("tag".into(), json!("proxy"));
    }
    json!({
        "log": {"level": "info"},
        "inbounds": [{"type": "mixed", "tag": "in", "listen": "127.0.0.1",
            "listen_port": 1870, "sniff": true}],
        "outbounds": [proxy, {"type": "direct", "tag": "direct"}],
        "route": {"final": "proxy", "auto_detect_interface": true},
        "experimental": {"clash_api": {"external_controller": "127.0.0.1:1872"}}
    })
}

/// Top-level Xray runtime config wrapping one outbound.
pub fn xray_config(outbound: &Value) -> Value {
    let mut proxy = outbound.clone();
    if let Some(o) = proxy.as_object_mut() {
        o.insert("tag".into(), json!("proxy"));
    }
    json!({
        "log": {"loglevel": "warning"},
        "inbounds": [
            {"protocol": "socks", "listen": "127.0.0.1", "port": 1870,
             "settings": {"udp": true}},
            {"protocol": "http", "listen": "127.0.0.1", "port": 1871}
        ],
        "outbounds": [proxy, {"protocol": "freedom"}]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vless_tls_ws() {
        let p = parse_link("vless://11111111-2222-4333-8444-555555555555@example.com:443?encryption=none&security=tls&sni=example.com&fp=chrome&type=ws&path=%2Fhub#Test%20Node").unwrap();
        assert_eq!(p.engine, Engine::SingBox);
        assert_eq!(p.name, "Test Node");
        assert_eq!(p.server_port, 443);
        assert_eq!(p.outbound["uuid"], json!("11111111-2222-4333-8444-555555555555"));
        assert_eq!(p.outbound["tls"]["server_name"], json!("example.com"));
        assert_eq!(p.outbound["transport"]["type"], json!("ws"));
    }

    #[test]
    fn vless_reality_goes_xray() {
        let p = parse_link("vless://11111111-2222-4333-8444-555555555555@example.com:443?encryption=none&security=reality&sni=www.microsoft.com&fp=chrome&pbk=ABCDEF&sid=1234&flow=xtls-rprx-vision#R").unwrap();
        assert_eq!(p.engine, Engine::Xray);
        assert_eq!(p.outbound["protocol"], json!("vless"));
        assert_eq!(p.outbound["streamSettings"]["security"], json!("reality"));
        assert_eq!(p.outbound["streamSettings"]["realitySettings"]["publicKey"], json!("ABCDEF"));
    }

    #[test]
    fn vmess_link() {
        use base64::Engine as _;
        let j = serde_json::json!({"v":"2","ps":"VM","add":"example.com","port":"443","id":"11111111-2222-4333-8444-555555555555","aid":"0","scy":"auto","net":"ws","type":"none","host":"example.com","path":"/w","tls":"tls","sni":"","alpn":"","fp":""});
        let b64 = base64::engine::general_purpose::STANDARD.encode(serde_json::to_vec(&j).unwrap());
        let p = parse_link(&format!("vmess://{b64}")).unwrap();
        assert_eq!(p.engine, Engine::SingBox);
        assert_eq!(p.name, "VM");
        assert_eq!(p.outbound["transport"]["type"], json!("ws"));
        assert!(p.outbound.get("tls").is_some());
    }

    #[test]
    fn trojan_link() {
        let p = parse_link("trojan://secret@example.com:443?sni=example.com#T").unwrap();
        assert_eq!(p.outbound["password"], json!("secret"));
        assert_eq!(p.outbound["tls"]["server_name"], json!("example.com"));
    }

    #[test]
    fn ss_sip002_and_legacy() {
        let a = parse_link("ss://YWVzLTI1Ni1nY206cGFzc3dvcmQ@example.com:8388#SS").unwrap();
        assert_eq!(a.outbound["method"], json!("aes-256-gcm"));
        assert_eq!(a.outbound["password"], json!("password"));
        let b = parse_link("ss://YWVzLTI1Ni1nY206cGFzc3dvcmRAMTI3LjAuMC4xOjgzODg=#SS2").unwrap();
        assert_eq!(b.server_host, "127.0.0.1");
        assert_eq!(b.server_port, 8388);
    }

    #[test]
    fn ss_plugin_refused() {
        let e = parse_link("ss://YWVzLTI1Ni1nY206cGFzc3dvcmQ@example.com:8388/?plugin=obfs").unwrap_err();
        assert!(matches!(e, LinkError::Unsupported(_)));
    }

    #[test]
    fn hy2_and_tuic() {
        let h = parse_link("hy2://pwd@example.com:443?sni=example.com&insecure=1#H").unwrap();
        assert_eq!(h.outbound["type"], json!("hysteria2"));
        assert_eq!(h.outbound["tls"]["insecure"], json!(true));
        let t = parse_link("tuic://11111111-2222-4333-8444-555555555555:pwd@example.com:443?sni=example.com#TU").unwrap();
        assert_eq!(t.outbound["uuid"], json!("11111111-2222-4333-8444-555555555555"));
    }

    #[test]
    fn wg_and_socks() {
        let w = parse_link("wireguard://PRIV@example.com:51820?publickey=PUB&allowedips=0.0.0.0%2F0#W").unwrap();
        assert_eq!(w.outbound["peer_public_key"], json!("PUB"));
        let s = parse_link("socks5://u:p@example.com:1080#S").unwrap();
        assert_eq!(s.outbound["username"], json!("u"));
    }

    #[test]
    fn recognized_but_engine_less() {
        assert!(matches!(parse_link("vpn://abc").unwrap_err(), LinkError::Unsupported(_)));
        assert!(matches!(parse_link("hysteria://u@example.com:443").unwrap_err(), LinkError::Unsupported(_)));
        assert!(matches!(parse_link("foo://x").unwrap_err(), LinkError::Unsupported(_)));
        assert!(matches!(parse_link("vless://nouuid@example.com").unwrap_err(), LinkError::Malformed(_)));
        // ssh:// and ikev2:// are first-class now (system client / OS VPN).
        let s = parse_link("ssh://u@example.com:22").unwrap();
        assert_eq!(s.engine, Engine::Ssh);
        let k = parse_link("ikev2://u:p@example.com#V").unwrap();
        assert_eq!(k.engine, Engine::Ikev2);
    }

    #[test]
    fn runtime_configs_shape() {
        let p = parse_link("trojan://secret@example.com:443#T").unwrap();
        let cfg = singbox_config(&p.outbound);
        assert_eq!(cfg["inbounds"][0]["listen_port"], json!(1870));
        assert_eq!(cfg["experimental"]["clash_api"]["external_controller"], json!("127.0.0.1:1872"));
        let x = xray_config(&p.outbound);
        assert_eq!(x["inbounds"][1]["port"], json!(1871));
    }
}

#[cfg(test)]
mod fix_tests {
    use super::*;

    #[test]
    fn plus_in_password_survives() {
        let p = parse_link("trojan://a+b@example.com:443#T").unwrap();
        assert_eq!(p.outbound["password"], serde_json::json!("a+b"));
        let q = parse_link("trojan://a%2Bb@example.com:443#T").unwrap();
        assert_eq!(q.outbound["password"], serde_json::json!("a+b"));
    }

    #[test]
    fn vmess_scheme_case_and_padding() {
        use base64::Engine as _;
        let j = serde_json::json!({"v":"2","ps":"X","add":"example.com","port":"443","id":"11111111-2222-4333-8444-555555555555","aid":"0","scy":"auto","net":"tcp","type":"none","host":"","path":"","tls":"","sni":"","alpn":"","fp":""});
        let full = base64::engine::general_purpose::STANDARD.encode(serde_json::to_vec(&j).unwrap());
        let no_pad = full.trim_end_matches('=');
        for link in [format!("vmess://{full}"), format!("VMESS://{no_pad}")] {
            let p = parse_link(&link).unwrap();
            assert_eq!(p.name, "X");
        }
    }

    #[test]
    fn ss_sip002_plugin_refused_cleanly() {
        let e = parse_link("ss://YWVzLTI1Ni1nY206cGFzc3dvcmQ@example.com:8388/?plugin=obfs-local%3Bobfs%3Dhttp#S").unwrap_err();
        assert!(matches!(e, LinkError::Unsupported(_)));
    }
}
