//! Subscription bodies, pasted JSON, .ovpn files → import items.
//!
//! Sources: base64 blob of links, multi-link text, sing-box JSON, Xray
//! JSON, Clash YAML (`proxies:` subset), OpenVPN `.ovpn` (cert-based).
//! Anything unrecognized comes back Skipped with a reason — never guessed.
use crate::links::{parse_link, Engine, LinkError, ParsedLink};
use serde_json::Value;

#[derive(Debug, Clone)]
pub enum ImportItem {
    Link(ParsedLink),
    RawJson {
        name: String,
        engine: Engine,
        json: Value,
        socks_port: u16,
    },
    Skipped {
        line: String,
        reason: String,
    },
}

pub fn link_error_reason(e: LinkError) -> String {
    match e {
        LinkError::Unsupported(s) => s,
        LinkError::Malformed(s) => format!("bad link: {s}"),
    }
}

/// Fetch a subscription URL (direct; 20s cap, 2MB cap).
pub async fn fetch_subscription(url: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("AetherGUI/0.7")
        .build()
        .map_err(|e| format!("http: {e}"))?;
    let res = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("fetch: {e}"))?;
    if !res.status().is_success() {
        return Err(format!("http {}", res.status()));
    }
    let bytes = res.bytes().await.map_err(|e| format!("read: {e}"))?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err("subscription too large".into());
    }
    String::from_utf8(bytes.to_vec()).map_err(|_| "not utf-8".into())
}

/// Split a subscription body into import items. Tries, in order: full
/// JSON, base64 blob of links, Clash YAML, plain multi-link text.
pub fn parse_subscription_body(body: &str) -> Vec<ImportItem> {
    let t = body.trim();
    if t.is_empty() {
        return vec![];
    }
    if t.starts_with('{') {
        return match serde_json::from_str::<Value>(t) {
            Ok(v) => vec![json_item("subscription", v)],
            Err(e) => vec![ImportItem::Skipped {
                line: "<json>".into(),
                reason: format!("bad json: {e}"),
            }],
        };
    }
    // Base64 blob? Must decode to something link-like.
    if let Ok(decoded) = crate::links::b64_decode(t) {
        if let Ok(s) = String::from_utf8(decoded) {
            let st = s.trim();
            if st.contains("://") {
                return parse_link_lines(st);
            }
        }
    }
    // Clash YAML?
    if t.contains("proxies:") {
        return parse_clash(t);
    }
    parse_link_lines(t)
}

fn json_item(name: &str, v: Value) -> ImportItem {
    let engine = detect_json_engine(&v);
    let socks_port = extract_socks_port(&v).unwrap_or(1870);
    match engine {
        Some(e) => ImportItem::RawJson {
            name: name.to_string(),
            engine: e,
            json: v,
            socks_port,
        },
        None => ImportItem::Skipped {
            line: name.to_string(),
            reason: "unrecognized JSON (need sing-box or Xray shape)".into(),
        },
    }
}

/// sing-box outbounds carry `type`, Xray ones `protocol`.
pub fn detect_json_engine(v: &Value) -> Option<Engine> {
    let obs = v.get("outbounds")?.as_array()?;
    let first = obs.first()?;
    if first.get("protocol").and_then(|p| p.as_str()).is_some() {
        return Some(Engine::Xray);
    }
    if first.get("type").and_then(|p| p.as_str()).is_some() {
        return Some(Engine::SingBox);
    }
    None
}

fn extract_socks_port(v: &Value) -> Option<u16> {
    v.get("inbounds")?.as_array()?.iter().find_map(|ib| {
        let proto = ib.get("protocol").and_then(|p| p.as_str()).unwrap_or("");
        let typ = ib.get("type").and_then(|p| p.as_str()).unwrap_or("");
        if proto == "socks" || typ == "socks" || typ == "mixed" {
            ib.get("port")
                .and_then(|p| p.as_u64())
                .or_else(|| ib.get("listen_port").and_then(|p| p.as_u64()))
                .and_then(|p| u16::try_from(p).ok())
        } else {
            None
        }
    })
}

fn parse_link_lines(text: &str) -> Vec<ImportItem> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| match parse_link(l) {
            Ok(p) => ImportItem::Link(p),
            Err(e) => ImportItem::Skipped {
                line: l.chars().take(48).collect(),
                reason: link_error_reason(e),
            },
        })
        .collect()
}

/// Clash `proxies:` subset → synthetic links → the normal parser, so all
/// validation stays in one place. Unknown proxy types come back Skipped.
fn parse_clash(yaml: &str) -> Vec<ImportItem> {
    let v: serde_yaml::Value = match serde_yaml::from_str(yaml) {
        Ok(v) => v,
        Err(e) => {
            return vec![ImportItem::Skipped {
                line: "<yaml>".into(),
                reason: format!("bad yaml: {e}"),
            }]
        }
    };
    let proxies = match v.get("proxies").and_then(|p| p.as_sequence()) {
        Some(s) => s,
        None => {
            return vec![ImportItem::Skipped {
                line: "<yaml>".into(),
                reason: "no proxies: list".into(),
            }]
        }
    };
    proxies
        .iter()
        .map(|p| {
            let g = |k: &str| p.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
            let name = g("name");
            let link = clash_to_link(p);
            match link {
                Some(l) => match parse_link(&l) {
                    Ok(mut parsed) => {
                        if !name.is_empty() {
                            parsed.name = name;
                        }
                        ImportItem::Link(parsed)
                    }
                    Err(e) => ImportItem::Skipped {
                        line: name,
                        reason: link_error_reason(e),
                    },
                },
                None => ImportItem::Skipped {
                    line: name,
                    reason: format!(
                        "unsupported clash type: {}",
                        g("type")
                    ),
                },
            }
        })
        .collect()
}

fn clash_to_link(p: &serde_yaml::Value) -> Option<String> {
    let g = |k: &str| p.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let server = g("server");
    let port = p.get("port").and_then(|x| x.as_u64()).unwrap_or(0);
    if server.is_empty() || port == 0 || port > 65535 {
        return None;
    }
    let cipher = g("cipher");
    let tls = g("tls").eq_ignore_ascii_case("true");
    let sni = if g("servername").is_empty() { g("sni") } else { g("servername") };
    let fp = g("client-fingerprint");
    let net = g("network");
    match g("type").as_str() {
        "ss" => Some(format!("ss://{cipher}:{}@{server}:{port}", g("password"))),
        "vmess" => {
            use base64::Engine as _;
            let obj = serde_json::json!({
                "v": "2", "ps": g("name"), "add": server, "port": port.to_string(),
                "id": g("uuid"), "aid": g("alterId"), "scy": "auto",
                "net": if net.is_empty() { "tcp" } else { net.as_str() },
                "tls": if tls { "tls" } else { "" },
                "sni": sni, "fp": fp,
            });
            let b64 = base64::engine::general_purpose::STANDARD
                .encode(serde_json::to_vec(&obj).unwrap_or_default());
            Some(format!("vmess://{b64}"))
        }
        "trojan" => Some(format!(
            "trojan://{}@{server}:{port}?sni={sni}&fp={fp}#{}",
            g("password"),
            g("name")
        )),
        "vless" => {
            let reality = p.get("reality-opts").is_some();
            let mut q = format!(
                "encryption=none&security={}",
                if reality {
                    "reality"
                } else if tls {
                    "tls"
                } else {
                    "none"
                }
            );
            if !sni.is_empty() {
                q += &format!("&sni={sni}");
            }
            if !fp.is_empty() {
                q += &format!("&fp={fp}");
            }
            if reality {
                let r = p.get("reality-opts");
                let pbk = r.and_then(|x| x.get("public-key")).and_then(|x| x.as_str()).unwrap_or("");
                let sid = r.and_then(|x| x.get("short-id")).and_then(|x| x.as_str()).unwrap_or("");
                q += &format!("&pbk={pbk}&sid={sid}");
            }
            if !net.is_empty() && net != "tcp" {
                q += &format!("&type={net}");
                if net == "ws" {
                    let ws = p.get("ws-opts");
                    let path = ws.and_then(|x| x.get("path")).and_then(|x| x.as_str()).unwrap_or("/");
                    let host = ws
                        .and_then(|x| x.get("headers"))
                        .and_then(|x| x.get("Host"))
                        .and_then(|x| x.as_str())
                        .unwrap_or("");
                    q += &format!("&path={path}&host={host}");
                }
            }
            if !g("flow").is_empty() {
                q += &format!("&flow={}", g("flow"));
            }
            Some(format!("vless://{}@{server}:{port}?{q}#{}", g("uuid"), g("name")))
        }
        "hysteria2" => Some(format!(
            "hy2://{}@{server}:{port}?sni={sni}{}#{}",
            g("password"),
            if g("skip-cert-verify").eq_ignore_ascii_case("true") {
                "&insecure=1"
            } else {
                ""
            },
            g("name")
        )),
        "wireguard" => Some(format!(
            "wireguard://{}@{server}:{port}?publickey={}&allowedips=0.0.0.0%2F0#{}",
            g("private-key"),
            g("public-key"),
            g("name")
        )),
        "tuic" => Some(format!(
            "tuic://{}:{}@{server}:{port}?sni={sni}#{}",
            g("uuid"),
            g("password"),
            g("name")
        )),
        "socks5" => Some(format!(
            "socks5://{}:{}@{server}:{port}#{}",
            g("username"),
            g("password"),
            g("name")
        )),
        "http" => Some(format!(
            "http://{}:{}@{server}:{port}#{}",
            g("username"),
            g("password"),
            g("name")
        )),
        _ => None,
    }
}

/// Minimal .ovpn → sing-box openvpn outbound (cert-based). `auth-user-pass`
/// without inline credentials is refused loudly — stored plaintext
/// credentials would be a lie of security.
pub fn parse_ovpn(text: &str) -> Result<ParsedLink, LinkError> {
    let mut host = String::new();
    let mut port: u16 = 1194;
    let mut _proto = "udp".to_string();
    for line in text.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("remote ") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() >= 2 {
                host = parts[0].to_string();
                port = parts[1].parse().unwrap_or(1194);
                if parts.len() >= 3 {
                    _proto = parts[2].to_lowercase();
                }
            }
        } else if line.starts_with("proto ") {
            _proto = line["proto ".len()..].trim().to_lowercase();
        } else if line == "auth-user-pass" {
            return Err(LinkError::Unsupported(
                "ovpn needs inline username/password (auth-user-pass)".into(),
            ));
        }
    }
    if host.is_empty() {
        return Err(LinkError::Malformed("no remote in ovpn".into()));
    }
    Ok(ParsedLink {
        name: "ovpn".to_string(),
        engine: Engine::SingBox,
        outbound: serde_json::json!({
            "type": "openvpn", "tag": "proxy",
            "server": host, "server_port": port,
        }),
        server_host: host,
        server_port: port,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sub_multi_link_with_junk() {
        let items = parse_subscription_body("vless://u@example.com:443# A\n\nnot-a-link\ntrojan://p@example.com:443# B");
        assert_eq!(items.len(), 3);
        assert!(matches!(items[0], ImportItem::Link(_)));
        assert!(matches!(items[1], ImportItem::Skipped { .. }));
    }

    #[test]
    fn sub_base64_blob() {
        use base64::Engine as _;
        let blob = base64::engine::general_purpose::STANDARD.encode("ss://YWVzLTI1Ni1nY206cGFzc3dvcmQ@example.com:8388#S");
        let items = parse_subscription_body(&blob);
        assert_eq!(items.len(), 1);
        assert!(matches!(items[0], ImportItem::Link(_)));
    }

    #[test]
    fn sub_json_detects_engine() {
        let sb = serde_json::json!({"outbounds": [{"type": "vless"}], "inbounds": [{"type": "mixed", "listen_port": 1080}]});
        match &parse_subscription_body(&sb.to_string())[0] {
            ImportItem::RawJson { engine, socks_port, .. } => {
                assert_eq!(*engine, Engine::SingBox);
                assert_eq!(*socks_port, 1080);
            }
            other => panic!("unexpected {other:?}"),
        }
        let xr = serde_json::json!({"outbounds": [{"protocol": "vless"}]});
        match &parse_subscription_body(&xr.to_string())[0] {
            ImportItem::RawJson { engine, .. } => assert_eq!(*engine, Engine::Xray),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn clash_subset_imports() {
        let yml = "proxies:\n  - {name: A, type: ss, server: 1.2.3.4, port: 8388, cipher: aes-256-gcm, password: pw}\n  - {name: B, type: snell, server: 1.2.3.4, port: 8388}\n";
        let items = parse_subscription_body(yml);
        assert_eq!(items.len(), 2);
        assert!(matches!(items[0], ImportItem::Link(_)));
        assert!(matches!(items[1], ImportItem::Skipped { .. }));
    }

    #[test]
    fn ovpn_cert_based() {
        let items = parse_ovpn("client\nremote vpn.example.com 1194 udp\n<ca>\nX\n</ca>\n");
        assert!(items.is_ok());
        let p = items.unwrap();
        assert_eq!(p.server_host, "vpn.example.com");
        assert_eq!(p.server_port, 1194);
    }

    #[test]
    fn ovpn_auth_user_pass_refused() {
        let e = parse_ovpn("client\nremote vpn.example.com 1194\nauth-user-pass\n").unwrap_err();
        assert!(matches!(e, LinkError::Unsupported(_)));
    }
}
