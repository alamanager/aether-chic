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
    }
}

pub fn manual_addrs() -> (String, String) {
    (
        format!("127.0.0.1:{MANUAL_SOCKS_PORT}"),
        format!("127.0.0.1:{MANUAL_HTTP_PORT}"),
    )
}

fn resolve_sidecar(app: &AppHandle, kind: SidecarKind) -> Result<PathBuf, AetherError> {
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
    let cfg_path = manual_dir.join(match kind {
        SidecarKind::SingBox => "sing-box.json",
        SidecarKind::Xray => "xray.json",
    });
    std::fs::write(&cfg_path, config_json).map_err(|e| AetherError::Internal(e.to_string()))?;

    let mut cmd = std::process::Command::new(&binary);
    match kind {
        SidecarKind::SingBox => {
            cmd.arg("run").arg("-c").arg(&cfg_path);
        }
        SidecarKind::Xray => {
            cmd.arg("run").arg("-c").arg(&cfg_path);
        }
    }
    let mut session = ChildSession::spawn(cmd).map_err(|e| AetherError::Internal(e.to_string()))?;
    let stdout = session.take_stdout();
    let stderr = session.take_stderr();
    {
        let mut mgr = manager.lock().unwrap();
        mgr.session = Some(Session::Child(session));
    }
    for (stream, tag) in [(stdout, "out"), (stderr, "err")] {
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
    std::thread::spawn(move || loop {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
