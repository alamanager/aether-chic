//! Live client counts per proxy endpoint.
//!
//! Counts ESTABLISHED TCP connections whose local port is one of ours
//! (SOCKS / HTTP frontend / Tor-Psiphon exits) via Get-NetTCPConnection.
//! The cmdlet returns enum values (culture-invariant), so unlike
//! `netstat` this is not broken by localized Windows builds. Runs hidden
//! via cmd::silent; failures degrade to empty lists, never an error popup.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;
use tauri::AppHandle;

#[derive(Debug, Clone, Serialize)]
pub struct ClientEntry {
    pub ip: String,
    pub connections: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct EndpointClients {
    pub label: String,
    pub port: u16,
    /// Distinct non-loopback remote IPs with their connection counts.
    pub clients: Vec<ClientEntry>,
    /// Connections from this machine itself (loopback) — not "people".
    pub local_connections: usize,
    pub total_connections: usize,
}

#[derive(Debug, Clone, Deserialize)]
struct TcpRow {
    #[serde(rename = "LocalPort")]
    local_port: u16,
    #[serde(rename = "RemoteAddress")]
    remote_address: String,
}

fn parse_rows(json: &str) -> Vec<TcpRow> {
    let v: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let arr = match v {
        serde_json::Value::Array(a) => a,
        single => vec![single],
    };
    arr.into_iter().filter_map(|r| serde_json::from_value(r).ok()).collect()
}

fn is_loopback(ip: &str) -> bool {
    ip == "127.0.0.1" || ip == "::1" || ip.starts_with("127.")
}

fn summarize(label: &str, port: u16, rows: &[TcpRow]) -> EndpointClients {
    let mut per_ip: HashMap<String, usize> = HashMap::new();
    let mut local_connections = 0usize;
    for r in rows.iter().filter(|r| r.local_port == port) {
        if is_loopback(&r.remote_address) {
            local_connections += 1;
        } else {
            *per_ip.entry(r.remote_address.clone()).or_insert(0) += 1;
        }
    }
    let mut clients: Vec<ClientEntry> = per_ip
        .into_iter()
        .map(|(ip, connections)| ClientEntry { ip, connections })
        .collect();
    // Busiest first — the heaviest user is who you check first.
    clients.sort_by(|a, b| b.connections.cmp(&a.connections).then(a.ip.cmp(&b.ip)));
    if clients.len() > 32 {
        clients.truncate(32);
    }
    let total_connections = local_connections + clients.iter().map(|c| c.connections).sum::<usize>();
    EndpointClients {
        label: label.to_string(),
        port,
        clients,
        local_connections,
        total_connections,
    }
}

fn query_table() -> Vec<TcpRow> {
    let mut cmd = Command::new("powershell");
    crate::cmd::silent(&mut cmd);
    let out = cmd
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Get-NetTCPConnection -State Established -ErrorAction SilentlyContinue | Select-Object LocalPort,RemoteAddress | ConvertTo-Json -Compress -Depth 2",
        ])
        .output();
    match out {
        Ok(o) if o.status.success() => parse_rows(&String::from_utf8_lossy(&o.stdout)),
        _ => Vec::new(),
    }
}

#[tauri::command]
pub fn proxy_clients(app: AppHandle) -> Vec<EndpointClients> {
    let profile = crate::aether::profiles::load(&app);
    let socks_port = profile
        .bind_address
        .parse::<std::net::SocketAddr>()
        .map(|s| s.port())
        .unwrap_or(1819);
    let mut watched: Vec<(String, u16)> = vec![
        ("SOCKS5".to_string(), socks_port),
        ("HTTP".to_string(), profile.frontend_http_addr().port()),
    ];
    if let Some(a) = profile.tor_exit_addr() {
        watched.push(("Tor exit".to_string(), a.port()));
    }
    if let Some(a) = profile.psi_exit_addr() {
        watched.push(("Psiphon exit".to_string(), a.port()));
    }
    // Same port can serve two roles (e.g. custom binds); report once.
    watched.sort_by_key(|(_, p)| *p);
    watched.dedup_by_key(|(_, p)| *p);
    let rows = query_table();
    watched
        .iter()
        .map(|(label, port)| summarize(label, *port, &rows))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(port: u16, ip: &str) -> TcpRow {
        TcpRow {
            local_port: port,
            remote_address: ip.to_string(),
        }
    }

    #[test]
    fn groups_by_ip_and_skips_loopback() {
        let rows = vec![
            row(1819, "192.168.1.5"),
            row(1819, "192.168.1.5"),
            row(1819, "192.168.1.9"),
            row(1819, "127.0.0.1"),
            row(1820, "192.168.1.5"),
        ];
        let ep = summarize("SOCKS5", 1819, &rows);
        assert_eq!(ep.total_connections, 4);
        assert_eq!(ep.local_connections, 1);
        assert_eq!(ep.clients.len(), 2);
        assert_eq!(ep.clients[0].ip, "192.168.1.5");
        assert_eq!(ep.clients[0].connections, 2);
    }

    #[test]
    fn single_object_json_parses() {
        let rows = parse_rows(r#"{"LocalPort":1819,"RemoteAddress":"10.0.0.2"}"#);
        assert_eq!(rows.len(), 1);
        assert_eq!(parse_rows("not json").len(), 0);
    }
}
