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
    /// Stable per-connection id (port:ip:port) for delta tracking.
    pub conn_id: String,
    /// Cumulative connection bytes (ESTATS, elevated only).
    pub bytes_up: Option<u64>,
    pub bytes_down: Option<u64>,
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
    /// True when byte counters were actually collected (elevated).
    pub metered: bool,
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

/// One observed connection. `up`/`down` are its cumulative ESTATS bytes
/// (elevated only); without them each sample just counts 1 connection.
#[derive(Debug, Clone)]
struct ConnSample {
    ip: String,
    conn_id: String,
    up: Option<u64>,
    down: Option<u64>,
}

fn group(label: &str, port: u16, conns: Vec<ConnSample>, metered: bool) -> EndpointClients {
    let mut per_ip: HashMap<String, Vec<ConnSample>> = HashMap::new();
    let mut local_connections = 0usize;
    for c in conns {
        if is_loopback(&c.ip) {
            local_connections += 1;
        } else {
            per_ip.entry(c.ip.clone()).or_default().push(c);
        }
    }
    let mut clients: Vec<ClientEntry> = per_ip
        .into_iter()
        .map(|(ip, v)| {
            let (up, down) = fold_bytes(&v);
            ClientEntry {
                connections: v.len(),
                conn_id: format!("{}:{}", ip, v.len()),
                ip,
                bytes_up: up,
                bytes_down: down,
            }
        })
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
        metered,
    }
}

/// Sum byte counters when every sample of the group has them; otherwise
/// None (never mix counted and uncounted — that number would lie).
fn fold_bytes(v: &[ConnSample]) -> (Option<u64>, Option<u64>) {
    if v.iter().all(|c| c.up.is_some() && c.down.is_some()) {
        let up = v.iter().map(|c| c.up.unwrap_or(0)).fold(0u64, |a, b| a.saturating_add(b));
        let down = v.iter().map(|c| c.down.unwrap_or(0)).fold(0u64, |a, b| a.saturating_add(b));
        (Some(up), Some(down))
    } else {
        (None, None)
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
    if is_elevated() {
        if let Some(by_port) = estats_snapshot(&watched) {
            return watched
                .iter()
                .map(|(label, port)| {
                    group(label, *port, by_port.get(port).cloned().unwrap_or_default(), true)
                })
                .collect();
        }
        // ESTATS refused at runtime despite elevation — fall through.
    }
    let rows = query_table();
    watched
        .iter()
        .map(|(label, port)| {
            let conns: Vec<ConnSample> = rows
                .iter()
                .filter(|r| r.local_port == *port)
                .map(|r| ConnSample {
                    conn_id: r.remote_address.clone(),
                    ip: r.remote_address.clone(),
                    up: None,
                    down: None,
                })
                .collect();
            group(label, *port, conns, false)
        })
        .collect()
}

/// Whether this process runs elevated. Decides if per-connection byte
/// counters (ESTATS) can work at all — unelevated they silently cannot,
/// and every byte field stays None by design.
#[tauri::command]
pub fn is_elevated() -> bool {
    #[cfg(windows)]
    {
        unsafe { IsUserAnAdmin() != 0 }
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[cfg(windows)]
#[link(name = "shell32")]
extern "system" {
    fn IsUserAnAdmin() -> i32;
}

/// Relaunch this app elevated (UAC prompt). Handshake file proves the
/// elevated copy actually booted: if the user declines UAC nothing
/// appears and this process keeps running — declining never kills you.
/// On success this process exits a second later and the elevated copy
/// takes over (it never auto-connects, so no port fight).
#[tauri::command]
pub fn request_admin() -> bool {
    if is_elevated() {
        return true;
    }
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return false,
    };
    let flag = std::env::temp_dir().join(format!("aether-elevated-{}.ready", std::process::id()));
    let _ = std::fs::remove_file(&flag);
    let mut cmd = Command::new("powershell");
    crate::cmd::silent(&mut cmd);
    let spawned = cmd
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &format!(
                "Start-Process '{}' -Verb RunAs -ArgumentList '--elevated-ready={}'",
                exe.display(),
                flag.display()
            ),
        ])
        .spawn();
    if spawned.is_err() {
        return false;
    }
    for _ in 0..150 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if flag.exists() {
            let _ = std::fs::remove_file(&flag);
            let flag2 = flag.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(1));
                let _ = flag2;
                std::process::exit(0);
            });
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Per-connection byte counters via IP Helper ESTATS (elevated only).
// Research verdict: unelevated Get returns garbage/undefined — never
// trust it. Every failure here degrades to None, never a crash.
// ---------------------------------------------------------------------------

#[cfg(windows)]
#[link(name = "iphlpapi")]
extern "system" {
    fn GetExtendedTcpTable(
        table: *mut std::ffi::c_void,
        size: *mut u32,
        order: i32,
        af: u32,
        table_class: u32,
        reserved: u32,
    ) -> u32;
    fn GetPerTcpConnectionEStats(
        row: *const MibTcpRow,
        estats_type: u32,
        rw: *const DataRw,
        rw_version: u32,
        rw_size: u32,
        ros: *const u8,
        ros_version: u32,
        ros_size: u32,
        rod: *mut DataRod,
        rod_version: u32,
        rod_size: u32,
    ) -> u32;
    fn SetPerTcpConnectionEStats(
        row: *const MibTcpRow,
        estats_type: u32,
        rw: *const DataRw,
        rw_version: u32,
        rw_size: u32,
        ros: *const u8,
        ros_version: u32,
        ros_size: u32,
    ) -> u32;
    fn GetPerTcp6ConnectionEStats(
        row: *const MibTcp6Row,
        estats_type: u32,
        rw: *const DataRw,
        rw_version: u32,
        rw_size: u32,
        ros: *const u8,
        ros_version: u32,
        ros_size: u32,
        rod: *mut DataRod,
        rod_version: u32,
        rod_size: u32,
    ) -> u32;
    fn SetPerTcp6ConnectionEStats(
        row: *const MibTcp6Row,
        estats_type: u32,
        rw: *const DataRw,
        rw_version: u32,
        rw_size: u32,
        ros: *const u8,
        ros_version: u32,
        ros_size: u32,
    ) -> u32;
}

#[cfg(windows)]
#[repr(C)]
struct MibTcpRow {
    state: u32,
    local_addr: u32,
    local_port: u32,
    remote_addr: u32,
    remote_port: u32,
}

#[cfg(windows)]
#[repr(C)]
struct MibTcp6Row {
    state: u32,
    local_scope: u32,
    local_addr: [u8; 16],
    local_port: u32,
    remote_scope: u32,
    remote_addr: [u8; 16],
    remote_port: u32,
}

#[cfg(windows)]
#[repr(C)]
struct DataRw {
    enable: u8,
}

#[cfg(windows)]
#[repr(C)]
struct DataRod {
    data_bytes_out: u64,
    data_segs_out: u64,
    data_bytes_in: u64,
    data_segs_in: u64,
    segs_out: u32,
    segs_in: u32,
    soft_errors: u32,
    soft_error_reason: u32,
    snd_una: u32,
    snd_nxt: u32,
    snd_max: u32,
    thru_bytes_acked: u32,
    rcv_nxt: u32,
    thru_bytes_received: u32,
}

#[cfg(windows)]
const AF_INET: u32 = 2;
#[cfg(windows)]
const AF_INET6: u32 = 23;
#[cfg(windows)]
const TABLE_OWNER_PID_ALL: u32 = 5;
#[cfg(windows)]
const ESTATS_DATA: u32 = 1;
#[cfg(windows)]
const NO_ERROR: u32 = 0;

/// Network-order DWORD low half → host port. Pure math, no platform
/// dependency, so it stays ungated and tested everywhere.
fn port_host(raw: u32) -> u16 {
    u16::from_be((raw & 0xFFFF) as u16)
}

#[cfg(windows)]
fn read_table(af: u32) -> Vec<u8> {
    unsafe {
        let mut size: u32 = 0;
        GetExtendedTcpTable(
            std::ptr::null_mut(),
            &mut size,
            0,
            af,
            TABLE_OWNER_PID_ALL,
            0,
        );
        // Sanity cap: a connection table past 8MB is not ours to walk.
        if size == 0 || size > 8 * 1024 * 1024 {
            return Vec::new();
        }
        let mut buf = vec![0u8; size as usize];
        let rc = GetExtendedTcpTable(
            buf.as_mut_ptr() as *mut std::ffi::c_void,
            &mut size,
            0,
            af,
            TABLE_OWNER_PID_ALL,
            0,
        );
        if rc != NO_ERROR {
            return Vec::new();
        }
        buf.truncate(size as usize);
        buf
    }
}

/// Snapshot watched ports → samples. None when ESTATS is unavailable
/// (not elevated, disabled, or any failure) — caller falls back.
#[cfg(windows)]
fn estats_snapshot(watched: &[(String, u16)]) -> Option<HashMap<u16, Vec<ConnSample>>> {
    let ports: std::collections::HashSet<u16> = watched.iter().map(|(_, p)| *p).collect();
    let mut out: HashMap<u16, Vec<ConnSample>> = HashMap::new();
    let mut seen_rows = 0usize;
    let rw = DataRw { enable: 1 };

    // IPv4.
    let buf4 = read_table(AF_INET);
    if buf4.len() >= 4 {
        let n = u32::from_ne_bytes([buf4[0], buf4[1], buf4[2], buf4[3]]) as usize;
        let row_size = std::mem::size_of::<MibTcpRow>();
        for i in 0..n {
            let off = 4 + i.saturating_mul(row_size);
            if off.saturating_add(row_size) > buf4.len() || seen_rows > 4096 {
                break;
            }
            seen_rows += 1;
            let row = unsafe { &*(buf4.as_ptr().add(off) as *const MibTcpRow) };
            let lp = port_host(row.local_port);
            if !ports.contains(&lp) {
                continue;
            }
            let rip = std::net::Ipv4Addr::from(u32::from_be(row.remote_addr)).to_string();
            let rp = port_host(row.remote_port);
            // Enable collection, then read. Either may refuse — skip then.
            let bytes = unsafe {
                if SetPerTcpConnectionEStats(
                    row,
                    ESTATS_DATA,
                    &rw,
                    0,
                    std::mem::size_of::<DataRw>() as u32,
                    std::ptr::null(),
                    0,
                    0,
                ) != NO_ERROR
                {
                    None
                } else {
                    let mut rod: DataRod = std::mem::zeroed();
                    if GetPerTcpConnectionEStats(
                        row,
                        ESTATS_DATA,
                        &rw,
                        0,
                        std::mem::size_of::<DataRw>() as u32,
                        std::ptr::null(),
                        0,
                        0,
                        &mut rod,
                        0,
                        std::mem::size_of::<DataRod>() as u32,
                    ) != NO_ERROR
                    {
                        None
                    } else {
                        Some((rod.data_bytes_out, rod.data_bytes_in))
                    }
                }
            };
            let (up, down) = match bytes {
                Some((o, i)) => (Some(o), Some(i)),
                None => (None, None),
            };
            out.entry(lp).or_default().push(ConnSample {
                ip: rip.clone(),
                conn_id: format!("tcp:{lp}:{rip}:{rp}"),
                up,
                down,
            });
        }
    }

    // IPv6.
    let buf6 = read_table(AF_INET6);
    if buf6.len() >= 4 {
        let n = u32::from_ne_bytes([buf6[0], buf6[1], buf6[2], buf6[3]]) as usize;
        let row_size = std::mem::size_of::<MibTcp6Row>();
        for i in 0..n {
            let off = 4 + i.saturating_mul(row_size);
            if off.saturating_add(row_size) > buf6.len() || seen_rows > 8192 {
                break;
            }
            seen_rows += 1;
            let row = unsafe { &*(buf6.as_ptr().add(off) as *const MibTcp6Row) };
            let lp = port_host(row.local_port);
            if !ports.contains(&lp) {
                continue;
            }
            let rip = std::net::Ipv6Addr::from(row.remote_addr).to_string();
            let rp = port_host(row.remote_port);
            let bytes = unsafe {
                if SetPerTcp6ConnectionEStats(
                    row,
                    ESTATS_DATA,
                    &rw,
                    0,
                    std::mem::size_of::<DataRw>() as u32,
                    std::ptr::null(),
                    0,
                    0,
                ) != NO_ERROR
                {
                    None
                } else {
                    let mut rod: DataRod = std::mem::zeroed();
                    if GetPerTcp6ConnectionEStats(
                        row,
                        ESTATS_DATA,
                        &rw,
                        0,
                        std::mem::size_of::<DataRw>() as u32,
                        std::ptr::null(),
                        0,
                        0,
                        &mut rod,
                        0,
                        std::mem::size_of::<DataRod>() as u32,
                    ) != NO_ERROR
                    {
                        None
                    } else {
                        Some((rod.data_bytes_out, rod.data_bytes_in))
                    }
                }
            };
            let (up, down) = match bytes {
                Some((o, i)) => (Some(o), Some(i)),
                None => (None, None),
            };
            out.entry(lp).or_default().push(ConnSample {
                ip: rip.clone(),
                conn_id: format!("tcp6:{lp}:{rip}:{rp}"),
                up,
                down,
            });
        }
    }
    Some(out)
}

#[cfg(not(windows))]
fn estats_snapshot(_watched: &[(String, u16)]) -> Option<HashMap<u16, Vec<ConnSample>>> {
    None
}

/// Best-effort device name for a LAN IP via reverse DNS. Home routers
/// and phones usually answer nothing — then this is None and the UI
/// shows the IP. Bounded by a hard timeout so a dead lookup can never
/// hang the card; callers must cache per IP (one lookup per address).
#[tauri::command]
pub fn resolve_host(ip: String) -> Option<String> {
    if is_loopback(&ip) || ip.trim().is_empty() {
        return None;
    }
    let ip = ip.trim().to_string();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut cmd = Command::new("powershell");
        crate::cmd::silent(&mut cmd);
        let out = cmd
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                &format!(
                    "[System.Net.Dns]::GetHostEntry('{ip}') | Select-Object HostName | ConvertTo-Json -Compress"
                ),
            ])
            .output();
        let _ = tx.send(out.ok());
    });
    let out = rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .ok()??;
    if !out.status.success() {
        return None;
    }
    parse_host_json(&String::from_utf8_lossy(&out.stdout))
}

fn parse_host_json(json: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let name = v.get("HostName")?.as_str()?.trim();
    if name.is_empty() {
        return None;
    }
    // A bare echo of the IP is not a name.
    if name.parse::<std::net::IpAddr>().is_ok() {
        return None;
    }
    Some(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_by_ip_and_skips_loopback() {
        let rows = vec![
            ConnSample { ip: "192.168.1.5".into(), conn_id: "a".into(), up: None, down: None },
            ConnSample { ip: "192.168.1.5".into(), conn_id: "b".into(), up: None, down: None },
            ConnSample { ip: "192.168.1.9".into(), conn_id: "c".into(), up: None, down: None },
            ConnSample { ip: "127.0.0.1".into(), conn_id: "d".into(), up: None, down: None },
        ];
        let ep = group("SOCKS5", 1819, rows, false);
        assert_eq!(ep.total_connections, 4);
        assert_eq!(ep.local_connections, 1);
        assert_eq!(ep.clients.len(), 2);
        assert_eq!(ep.clients[0].ip, "192.168.1.5");
        assert_eq!(ep.clients[0].connections, 2);
        assert!(!ep.metered);
    }

    #[test]
    fn bytes_fold_only_when_complete() {
        let full = vec![
            ConnSample { ip: "10.0.0.2".into(), conn_id: "a".into(), up: Some(100), down: Some(1000) },
            ConnSample { ip: "10.0.0.2".into(), conn_id: "b".into(), up: Some(50), down: Some(500) },
        ];
        let ep = group("HTTP", 1820, full, true);
        assert_eq!(ep.clients[0].bytes_up, Some(150));
        assert_eq!(ep.clients[0].bytes_down, Some(1500));
        // One unmetered sample poisons the sum — report None instead.
        let mixed = vec![
            ConnSample { ip: "10.0.0.3".into(), conn_id: "a".into(), up: Some(100), down: Some(1000) },
            ConnSample { ip: "10.0.0.3".into(), conn_id: "b".into(), up: None, down: None },
        ];
        let ep2 = group("HTTP", 1820, mixed, true);
        assert_eq!(ep2.clients[0].bytes_up, None);
    }

    #[test]
    fn port_host_converts_network_order() {
        // 1819 = 0x071B → on the wire (LE host) stored as 0x1B07.
        assert_eq!(port_host(0x1B07), 1819);
    }

    #[test]
    fn single_object_json_parses() {
        let rows = parse_rows(r#"{"LocalPort":1819,"RemoteAddress":"10.0.0.2"}"#);
        assert_eq!(rows.len(), 1);
        assert_eq!(parse_rows("not json").len(), 0);
    }

    #[test]
    fn host_json_parses() {
        assert_eq!(
            parse_host_json(r#"{"HostName":"phone.lan"}"#),
            Some("phone.lan".to_string())
        );
        // Echoed IPs and blanks are not names.
        assert_eq!(parse_host_json(r#"{"HostName":"192.168.1.5"}"#), None);
        assert_eq!(parse_host_json(r#"{"HostName":"  "}"#), None);
        assert_eq!(parse_host_json("garbage"), None);
    }
}
