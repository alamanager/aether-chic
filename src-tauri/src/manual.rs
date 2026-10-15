//! Manual engine sidecars (sing-box / Xray) for user-supplied configs.
//!
//! Unlike the Aether core these take a complete config file, so they run
//! as plain children (no PTY, no prompts) under the SAME manager — one
//! active engine at a time, same status/log events, same disconnect path.
use crate::aether::{set_state_and_emit, ChildSession, Session};
use crate::error::AetherError;
use crate::events::{now_millis, LogEvent, LOG_EVENT};
use crate::state::ConnectionState;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidecarKind {
    SingBox,
    Xray,
    Ssh,
}

/// Fixed loopback endpoints for manual mode (far from the auto range and
/// the matrix lanes). sing-box `mixed` serves SOCKS+HTTP on one port.
pub const MANUAL_SOCKS_PORT: u16 = 1870;
pub const MANUAL_HTTP_PORT: u16 = 1871;

pub fn binary_name(kind: SidecarKind) -> &'static str {
    match kind {
        SidecarKind::SingBox => {
            if cfg!(windows) {
                "sing-box.exe"
            } else {
                "sing-box"
            }
        }
        SidecarKind::Xray => {
            if cfg!(windows) {
                "xray.exe"
            } else {
                "xray"
            }
        }
        SidecarKind::Ssh => {
            if cfg!(windows) {
                "ssh.exe"
            } else {
                "ssh"
            }
        }
    }
}

pub fn manual_addrs() -> (String, String) {
    (
        format!("127.0.0.1:{MANUAL_SOCKS_PORT}"),
        format!("127.0.0.1:{MANUAL_HTTP_PORT}"),
    )
}

fn resolve_sidecar(app: &AppHandle, kind: SidecarKind) -> Result<PathBuf, AetherError> {
    // SSH rides the OS client (OpenSSH ships with Windows 10+, macOS and
    // Linux) — key-based only; passwords can't go through headless.
    if kind == SidecarKind::Ssh {
        for cand in [
            "C:\\Windows\\System32\\OpenSSH\\ssh.exe",
            "/usr/bin/ssh",
            "/bin/ssh",
        ] {
            let p = PathBuf::from(cand);
            if p.exists() {
                return Ok(p);
            }
        }
        return Err(AetherError::BinaryMissing("ssh client".into()));
    }
    let path = app
        .path()
        .resource_dir()
        .map_err(|e| AetherError::Internal(e.to_string()))?
        .join("binaries")
        .join(binary_name(kind));
    if !path.exists() {
        return Err(AetherError::BinaryMissing(path.display().to_string()));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755));
    }
    Ok(path)
}

/// Start a sidecar with a ready-made JSON config. Fails fast when busy
/// or a port is taken; transitions mirror the auto flow so the UI needs
/// no new states.
pub fn start_manual(
    app: AppHandle,
    manager: Arc<Mutex<crate::aether::AetherManager>>,
    kind: SidecarKind,
    config_json: &str,
) -> Result<(), AetherError> {
    let binary = resolve_sidecar(&app, kind)?;
    let (socks, _) = manual_addrs();
    let socks_addr: std::net::SocketAddr = socks
        .parse()
        .map_err(|e| AetherError::Internal(format!("bad manual addr: {e}")))?;
    {
        let mut mgr = manager.lock().unwrap();
        if !matches!(
            mgr.state,
            ConnectionState::Idle | ConnectionState::Error { .. }
        ) {
            return Err(AetherError::AlreadyRunning);
        }
        if crate::aether::status::port_is_live(&socks_addr) {
            return Err(AetherError::PortInUse(socks_addr.port()));
        }
        mgr.state = ConnectionState::Launching;
    }
    let _ = app.emit(
        crate::events::STATUS_EVENT,
        &ConnectionState::Launching,
    );

    // Config lives next to the profile (wiped only on uninstall).
    let data_dir = crate::aether::app_data_dir(&app);
    let manual_dir = data_dir.join("manual");
    std::fs::create_dir_all(&manual_dir).map_err(|e| AetherError::Internal(e.to_string()))?;

    let mut cmd = std::process::Command::new(&binary);
    if kind == SidecarKind::Ssh {
        #[derive(serde::Deserialize)]
        struct SshConf {
            host: String,
            port: u16,
            user: String,
            key: String,
        }
        let c: SshConf = serde_json::from_str(config_json)
            .map_err(|e| AetherError::Internal(format!("bad ssh config: {e}")))?;
        if c.host.is_empty() || c.user.is_empty() || c.key.trim().is_empty() {
            return Err(AetherError::Internal("ssh needs host, user and key".into()));
        }
        let key_path = manual_dir.join("ssh_key");
        std::fs::write(&key_path, c.key.trim()).map_err(|e| AetherError::Internal(e.to_string()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600));
        }
        cmd.args([
            "-D",
            "127.0.0.1:1870",
            "-N",
            "-i",
            &key_path.to_string_lossy().into_owned(),
            "-o",
            "BatchMode=yes",
            "-o",
            "StrictHostKeyChecking=accept-new",
            "-o",
            "ConnectTimeout=30",
            "-p",
            &c.port.to_string(),
            &format!("{}@{}", c.user, c.host),
        ]);
    } else {
        let cfg_path = manual_dir.join(match kind {
            SidecarKind::SingBox => "sing-box.json",
            SidecarKind::Xray => "xray.json",
            SidecarKind::Ssh => unreachable!("handled above"),
        });
        std::fs::write(&cfg_path, config_json).map_err(|e| AetherError::Internal(e.to_string()))?;
        cmd.arg("run").arg("-c").arg(&cfg_path);
    }
    let mut session = ChildSession::spawn(cmd).map_err(|e| AetherError::Internal(e.to_string()))?;
    let stdout = session.take_stdout();
    let stderr = session.take_stderr();
    {
        let mut mgr = manager.lock().unwrap();
        mgr.session = Some(Session::Child(session));
    }
    for (stream, tag) in [(stdout.map(|s| Box::new(s) as Box<dyn std::io::Read + Send>), "out"), (stderr.map(|s| Box::new(s) as Box<dyn std::io::Read + Send>), "err")] {
        if let Some(stream) = stream {
            let app = app.clone();
            std::thread::spawn(move || {
                use std::io::BufRead;
                for line in std::io::BufReader::new(stream).lines().map_while(Result::ok) {
                    let _ = app.emit(
                        LOG_EVENT,
                        &LogEvent {
                            line: format!("[sidecar:{tag}] {line}"),
                            timestamp: now_millis(),
                        },
                    );
                }
            });
        }
    }
    monitor_manual(app, manager, socks_addr);
    Ok(())
}

fn monitor_manual(
    app: AppHandle,
    manager: Arc<Mutex<crate::aether::AetherManager>>,
    socks_addr: std::net::SocketAddr,
) {
    std::thread::spawn(move || {
        // Backstop: a misconfigured sidecar that never opens its port
        // must fail loudly, not sit in Launching forever.
        let deadline = std::time::Instant::now() + Duration::from_secs(60);
        loop {
        std::thread::sleep(Duration::from_millis(400));
        let mut mgr = manager.lock().unwrap();
        if mgr.user_requested_stop {
            return;
        }
        if mgr.session.as_mut().and_then(|s| s.try_wait()).is_some() {
            mgr.session = None;
            drop(mgr);
            set_state_and_emit(
                &app,
                &manager,
                ConnectionState::Error {
                    message: "Sidecar exited unexpectedly".to_string(),
                    phase: "connecting".to_string(),
                },
            );
            return;
        }
        if crate::aether::status::port_is_live(&socks_addr) {
            let (socks, _) = manual_addrs();
            drop(mgr);
            set_state_and_emit(
                &app,
                &manager,
                ConnectionState::Connected {
                    socks_addr: socks,
                    connected_at_ms: now_millis(),
                },
            );
            monitor_manual_connected(app, manager, socks_addr);
            return;
        }
        if std::time::Instant::now() >= deadline {
            mgr.session = None;
            drop(mgr);
            set_state_and_emit(
                &app,
                &manager,
                ConnectionState::Error {
                    message: "Sidecar did not open its port (bad config?)".to_string(),
                    phase: "connecting".to_string(),
                },
            );
            return;
        }
        }
    });
}

fn monitor_manual_connected(
    app: AppHandle,
    manager: Arc<Mutex<crate::aether::AetherManager>>,
    socks_addr: std::net::SocketAddr,
) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(2000));
        let mut mgr = manager.lock().unwrap();
        if mgr.user_requested_stop {
            return;
        }
        if mgr.session.as_mut().and_then(|s| s.try_wait()).is_some() {
            mgr.session = None;
            drop(mgr);
            set_state_and_emit(
                &app,
                &manager,
                ConnectionState::Error {
                    message: "Sidecar exited unexpectedly".to_string(),
                    phase: "connected".to_string(),
                },
            );
            return;
        }
        if !crate::aether::status::port_is_live(&socks_addr) {
            // Port went quiet: the sidecar died without an exit status we
            // could read (or its listener moved) — surface, don't spin.
            mgr.session = None;
            drop(mgr);
            set_state_and_emit(
                &app,
                &manager,
                ConnectionState::Error {
                    message: "Sidecar stopped answering".to_string(),
                    phase: "connected".to_string(),
                },
            );
            return;
        }
    });
}

/// Import DTOs for pasted text / files (one string in, items out).
#[derive(Debug, Clone, serde::Serialize)]
pub struct ImportDto {
    pub kind: String,
    pub name: String,
    pub engine: String,
    pub outbound: Option<serde_json::Value>,
    pub raw_json: Option<serde_json::Value>,
    pub socks_port: u16,
    pub host: String,
    pub port: u16,
    pub reason: String,
}

fn engine_str(e: crate::links::Engine) -> &'static str {
    match e {
        crate::links::Engine::SingBox => "singbox",
        crate::links::Engine::Xray => "xray",
        crate::links::Engine::Ssh => "ssh",
        crate::links::Engine::Ikev2 => "ikev2",
    }
}

#[tauri::command]
pub fn manual_import_text(text: String) -> Vec<ImportDto> {
    crate::subs::parse_subscription_body(&text)
        .into_iter()
        .map(|item| match item {
            crate::subs::ImportItem::Link(p) => ImportDto {
                kind: "link".into(),
                name: p.name.clone(),
                engine: engine_str(p.engine).into(),
                outbound: Some(p.outbound.clone()),
                raw_json: None,
                socks_port: 1870,
                host: p.server_host.clone(),
                port: p.server_port,
                reason: String::new(),
            },
            crate::subs::ImportItem::RawJson { name, engine, json, socks_port } => ImportDto {
                kind: "rawjson".into(),
                name,
                engine: engine_str(engine).into(),
                outbound: None,
                raw_json: Some(json),
                socks_port,
                host: String::new(),
                port: 0,
                reason: String::new(),
            },
            crate::subs::ImportItem::Skipped { line, reason } => ImportDto {
                kind: "skipped".into(),
                name: line,
                engine: String::new(),
                outbound: None,
                raw_json: None,
                socks_port: 0,
                host: String::new(),
                port: 0,
                reason,
            },
        })
        .collect()
}

#[tauri::command]
pub async fn manual_import_sub(url: String) -> Result<Vec<ImportDto>, String> {
    let body = crate::subs::fetch_subscription(&url).await?;
    Ok(manual_import_text(body))
}

#[tauri::command]
pub fn manual_import_ovpn(text: String) -> ImportDto {
    match crate::subs::parse_ovpn(&text) {
        Ok(p) => ImportDto {
            kind: "link".into(),
            name: p.name.clone(),
            engine: engine_str(p.engine).into(),
            outbound: Some(p.outbound.clone()),
            raw_json: None,
            socks_port: 1870,
            host: p.server_host.clone(),
            port: p.server_port,
            reason: String::new(),
        },
        Err(e) => ImportDto {
            kind: "skipped".into(),
            name: "ovpn".into(),
            engine: String::new(),
            outbound: None,
            raw_json: None,
            socks_port: 0,
            host: String::new(),
            port: 0,
            reason: crate::subs::link_error_reason(e),
        },
    }
}

#[tauri::command]
pub fn manual_connect(
    app: AppHandle,
    state: tauri::State<crate::state::AppState>,
    engine: String,
    config_json: String,
) -> Result<(), AetherError> {
    let kind = match engine.as_str() {
        "xray" => SidecarKind::Xray,
        "ssh" => SidecarKind::Ssh,
        "singbox" => SidecarKind::SingBox,
        _ => return Err(AetherError::Internal("unknown engine".into())),
    };
    if serde_json::from_str::<serde_json::Value>(&config_json).is_err() {
        return Err(AetherError::Internal("bad config json".into()));
    }
    start_manual(app, state.manager.clone(), kind, &config_json)
}

/// TCP handshake timing for latency badges. Resolves names, tries each
/// address, returns the fastest in ms. No traffic shaping games — a SYN
/// that answers is a reachable server.
#[tauri::command]
pub fn tcp_ping(host: String, port: u16) -> Result<u64, String> {
    use std::net::ToSocketAddrs;
    use std::time::Instant;
    let addrs: Vec<std::net::SocketAddr> = format!("{host}:{port}")
        .to_socket_addrs()
        .map_err(|e| format!("dns: {e}"))?
        .collect();
    if addrs.is_empty() {
        return Err("no address".into());
    }
    let mut best: Option<u64> = None;
    for a in addrs {
        let t = Instant::now();
        match std::net::TcpStream::connect_timeout(&a, Duration::from_secs(5)) {
            Ok(_) => {
                let ms = u64::try_from(t.elapsed().as_millis()).unwrap_or(u64::MAX);
                best = Some(best.map_or(ms, |b: u64| b.min(ms)));
            }
            Err(_) => continue,
        }
    }
    best.ok_or_else(|| "unreachable".into())
}

/// IKEv2 via the OS client (Windows: built-in VPN + rasdial; EAP-MSCHAPv2
/// user/pass only — PSK/cert need different stores, like ZedSecure).
/// No SOCKS port exists (system-wide tunnel); the placeholder session
/// below only feeds the shared disconnect path. Other platforms: refused
/// with a clear message until their backends land.
static IKEV2_ENTRY: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

pub(crate) fn ikev2_active() -> bool {
    IKEV2_ENTRY.lock().unwrap().is_some()
}

fn ps_run(script: &str) -> Result<String, String> {
    let mut cmd = std::process::Command::new("powershell");
    crate::cmd::silent(&mut cmd);
    let out = cmd
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .map_err(|e| format!("spawn: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

fn ikev2_name(name: &str) -> String {
    let clean: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    let clean = clean.trim_matches('-').to_string();
    format!("Aether-{}", if clean.is_empty() { "ikev2" } else { &clean[..clean.len().min(40)] })
}

#[tauri::command]
pub fn manual_ikev2_up(
    app: AppHandle,
    state: tauri::State<crate::state::AppState>,
    server: String,
    user: String,
    pass: String,
    name: String,
) -> Result<(), AetherError> {
    #[cfg(not(windows))]
    {
        let _ = (&app, &state, server, user, pass, name);
        return Err(AetherError::Internal("IKEv2-OS is Windows-only for now".into()));
    }
    #[cfg(windows)]
    {
        if !crate::clients::is_elevated() {
            return Err(AetherError::Internal("IKEv2 needs admin (Windows VPN store)".into()));
        }
        if server.trim().is_empty() || user.is_empty() {
            return Err(AetherError::Internal("server and username required".into()));
        }
        {
            let mgr = state.manager.lock().unwrap();
            if !matches!(
                mgr.state,
                ConnectionState::Idle | ConnectionState::Error { .. }
            ) {
                return Err(AetherError::AlreadyRunning);
            }
        }
        let entry = ikev2_name(&name);
        let esc = |s: &str| s.replace('\'', "''");
        ps_run(&format!(
            "Remove-VpnConnection -Name '{}' -Force -ErrorAction SilentlyContinue",
            esc(&entry)
        ))
        .ok();
        ps_run(&format!(
            "Add-VpnConnection -Name '{}' -ServerAddress '{}' -TunnelType Ikev2 -AuthenticationMethod Eap -EncryptionLevel Required -SplitTunneling:$false -RememberCredential",
            esc(&entry),
            esc(server.trim())
        ))
        .map_err(AetherError::Internal)?;
        ps_run(&format!(
            "rasdial '{}' '{}' '{}'",
            esc(&entry),
            esc(&user),
            esc(&pass.replace('\'', "''"))
        ))
        .map_err(AetherError::Internal)?;
        // Placeholder session: feeds disconnect/shutdown paths; the real
        // signal is the OS connection state polled below.
        let mut holder =
            std::process::Command::new("powershell");
        crate::cmd::silent(&mut holder);
        holder.args(["-NoProfile", "-NonInteractive", "-Command", "while ($true) { Start-Sleep 60 }"]);
        let session = ChildSession::spawn(holder).map_err(|e| AetherError::Internal(e.to_string()))?;
        {
            let mut mgr = state.manager.lock().unwrap();
            mgr.session = Some(Session::Child(session));
            mgr.state = ConnectionState::Launching;
        }
        let _ = app.emit(
            crate::events::STATUS_EVENT,
            &ConnectionState::Launching,
        );
        *IKEV2_ENTRY.lock().unwrap() = Some(entry.clone());
        let manager = state.manager.clone();
        std::thread::spawn(move || {
            // Grace period: rasdial returns before the tunnel is queryable.
            std::thread::sleep(std::time::Duration::from_secs(8));
            // Connected once the OS reports it (no SOCKS to probe).
            let deadline = std::time::Instant::now() + Duration::from_secs(60);
            loop {
                std::thread::sleep(Duration::from_millis(2000));
                let mgr = manager.lock().unwrap();
                if mgr.user_requested_stop {
                    return;
                }
                let up = ps_run(&format!(
                    "(Get-VpnConnection -Name '{}' -ErrorAction SilentlyContinue).ConnectionStatus",
                    esc(&entry)
                ))
                .map(|s| s.eq_ignore_ascii_case("connected"))
                .unwrap_or(false);
                if up {
                    drop(mgr);
                    crate::aether::set_state_and_emit(
                        &app,
                        &manager,
                        ConnectionState::Connected {
                            socks_addr: "system-vpn".to_string(),
                            connected_at_ms: crate::events::now_millis(),
                        },
                    );
                    break;
                }
                if std::time::Instant::now() >= deadline {
                    drop(mgr);
                    ikev2_down_inner(&entry);
                    crate::aether::set_state_and_emit(
                        &app,
                        &manager,
                        ConnectionState::Error {
                            message: "IKEv2 did not come up".to_string(),
                            phase: "connecting".to_string(),
                        },
                    );
                    return;
                }
            }
            // Watchdog: surface drops.
            loop {
                std::thread::sleep(Duration::from_secs(5));
                let mgr = manager.lock().unwrap();
                if mgr.user_requested_stop {
                    return;
                }
                let up = ps_run(&format!(
                    "(Get-VpnConnection -Name '{}' -ErrorAction SilentlyContinue).ConnectionStatus",
                    esc(&entry)
                ))
                .map(|s| s.eq_ignore_ascii_case("connected"))
                .unwrap_or(false);
                if !up {
                    drop(mgr);
                    crate::aether::set_state_and_emit(
                        &app,
                        &manager,
                        ConnectionState::Error {
                            message: "IKEv2 dropped".to_string(),
                            phase: "connected".to_string(),
                        },
                    );
                    return;
                }
            }
        });
        return Ok(());
    }
}

fn ikev2_down_inner(entry: &str) {
    let esc = |s: &str| s.replace('\'', "''");
    ps_run(&format!("rasdial '{}' /disconnect", esc(entry))).ok();
    ps_run(&format!(
        "Remove-VpnConnection -Name '{}' -Force -ErrorAction SilentlyContinue",
        esc(entry)
    ))
    .ok();
    *IKEV2_ENTRY.lock().unwrap() = None;
}

/// Tear down an IKEv2 entry if one is up (app exit, disconnect BEFORE the
/// placeholder session dies — the OS tunnel outlives our process kill).
pub(crate) fn teardown_ikev2_if_active() {
    if let Some(entry) = IKEV2_ENTRY.lock().unwrap().clone() {
        ikev2_down_inner(&entry);
    }
}

#[tauri::command]
pub fn manual_ikev2_down() {
    teardown_ikev2_if_active();
}

#[cfg(test)]
mod tests {    use super::*;

    #[test]
    fn sidecar_binaries_per_platform() {
        let name = binary_name(SidecarKind::SingBox);
        let xray = binary_name(SidecarKind::Xray);
        if cfg!(windows) {
            assert_eq!(name, "sing-box.exe");
            assert_eq!(xray, "xray.exe");
        } else {
            assert_eq!(name, "sing-box");
            assert_eq!(xray, "xray");
        }
    }

    #[test]
    fn manual_addrs_are_fixed_loopback() {
        let (socks, http) = manual_addrs();
        assert_eq!(socks, "127.0.0.1:1870");
        assert_eq!(http, "127.0.0.1:1871");
    }

    #[test]
    fn tcp_ping_localhost() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let ms = tcp_ping("127.0.0.1".into(), port).unwrap();
        assert!(ms < 5000);
        drop(listener);
        // Closed port refuses fast — still an Err, never a hang.
        let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let shut = closed.local_addr().unwrap().port();
        drop(closed);
        assert!(tcp_ping("127.0.0.1".into(), shut).is_err());
    }
}
