//! Connection matrix lab: try every meaningful protocol × transport
//! combo for real (sequential connects with per-type budgets), measure
//! through-tunnel latency on each success, report per-combo results.
//!
//! The matrix is curated, not cartesian: reverses are MASQUE-only by
//! construction (covered by the masque rows), scan/noize/ip come from the
//! current profile so transports compare fairly. Fresh scan every combo
//! (quick_reconnect off) so results reflect reachability, not cache.
use crate::aether::profiles::{ConnectionProfile, ExtraTransport, Protocol};
use crate::error::AetherError;
use crate::events::{now_millis, MATRIX_EVENT};
use crate::state::{AppState, ConnectionState};
use serde::Serialize;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, State};

#[derive(Debug, Clone, Serialize)]
pub struct MatrixEvent {
    pub key: String,
    pub protocol: String,
    pub transport: String,
    /// "running" | "done" | "finished"
    pub phase: String,
    pub ok: bool,
    pub ms: Option<u64>,
    pub exit: Option<String>,
    pub note: String,
}

pub struct TestCombo {
    pub protocol: Protocol,
    pub transport: ExtraTransport,
    /// Seconds before this combo is declared failed.
    pub budget_secs: u64,
}

fn protocol_label(p: &Protocol) -> &'static str {
    match p {
        Protocol::Auto => "auto",
        Protocol::Masque => "masque",
        Protocol::Wireguard => "wireguard",
        Protocol::Gool => "gool",
        Protocol::GoolClassic => "gool-classic",
        Protocol::Mim => "mim",
    }
}

fn transport_label(t: &ExtraTransport) -> &'static str {
    match t {
        ExtraTransport::None => "warp",
        ExtraTransport::Tor => "tor",
        ExtraTransport::TorReverse => "tor-reverse",
        ExtraTransport::TorOnly => "tor-only",
        ExtraTransport::Psiphon => "psiphon",
        ExtraTransport::PsiphonReverse => "psiphon-reverse",
        ExtraTransport::PsiphonOnly => "psiphon-only",
    }
}

/// The curated matrix: 6 protocols × 5 transports = 30 real connects.
/// Tor bootstraps slowly (bridges, consensus), Psiphon tunnels in ~tens
/// of seconds, plain WARP scans in about a minute — budgets match that.
pub fn matrix_combos() -> Vec<TestCombo> {
    let protocols = [
        Protocol::Auto,
        Protocol::Masque,
        Protocol::Wireguard,
        Protocol::Gool,
        Protocol::GoolClassic,
        Protocol::Mim,
    ];
    let transports = [
        (ExtraTransport::None, 90u64),
        (ExtraTransport::Tor, 300u64),
        (ExtraTransport::TorOnly, 300u64),
        (ExtraTransport::Psiphon, 240u64),
        (ExtraTransport::PsiphonOnly, 240u64),
    ];
    let mut out = Vec::with_capacity(30);
    for p in protocols {
        for (t, b) in transports.iter() {
            out.push(TestCombo {
                protocol: p.clone(),
                transport: t.clone(),
                budget_secs: *b,
            });
        }
    }
    out
}

static CANCEL: AtomicBool = AtomicBool::new(false);

fn emit(app: &AppHandle, combo: &TestCombo, phase: &str, ok: bool, ms: Option<u64>, exit: Option<String>, note: String) {
    let ev = MatrixEvent {
        key: format!("{}+{}", protocol_label(&combo.protocol), transport_label(&combo.transport)),
        protocol: protocol_label(&combo.protocol).to_string(),
        transport: transport_label(&combo.transport).to_string(),
        phase: phase.to_string(),
        ok,
        ms,
        exit,
        note,
    };
    let _ = app.emit(MATRIX_EVENT, &ev);
}

/// Time a tiny fetch THROUGH the tunnel's SOCKS (user-facing latency,
/// not ICMP — ICMP often lies through tunnels). One sample: speed over
/// precision, the matrix already takes long enough.
fn measure_latency(socks_addr: &str) -> Option<u64> {
    #[cfg(windows)]
    const CURL: &str = "curl.exe";
    #[cfg(not(windows))]
    const CURL: &str = "curl";
    let mut cmd = std::process::Command::new(CURL);
    crate::cmd::silent(&mut cmd);
    let out = cmd
        .args([
            "-s",
            "-o",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
            "-w",
            "%{time_total}",
            "--max-time",
            "12",
            "-x",
            &format!("socks5h://{socks_addr}"),
            "https://www.cloudflare.com/cdn-cgi/trace",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let secs: f64 = s.parse().ok()?;
    if !secs.is_finite() || secs <= 0.0 {
        return None;
    }
    Some((secs * 1000.0).round() as u64)
}

fn is_busy(manager: &Arc<Mutex<crate::aether::AetherManager>>) -> bool {
    !matches!(
        manager.lock().unwrap().status(),
        ConnectionState::Idle | ConnectionState::Error { .. } | ConnectionState::Disconnecting
    )
}

#[tauri::command]
pub fn matrix_start(app: AppHandle, state: State<AppState>) -> Result<(), AetherError> {
    if is_busy(&state.manager) {
        return Err(AetherError::AlreadyRunning);
    }
    CANCEL.store(false, Ordering::SeqCst);
    let mut base = crate::aether::profiles::load(&app);
    // Fresh scan every combo: cached gateways would answer "what worked
    // last time", not "what reaches from here".
    base.quick_reconnect = false;
    let manager = state.manager.clone();
    let combos = matrix_combos();
    std::thread::spawn(move || {
        for combo in &combos {
            if CANCEL.load(Ordering::SeqCst) {
                break;
            }
            run_combo(&app, &manager, &base, combo);
            // Cooldown so the next bind never trips on our own TIME_WAIT.
            for _ in 0..3 {
                if CANCEL.load(Ordering::SeqCst) {
                    break;
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        }
        let _ = crate::aether::request_disconnect(&app, &manager);
        let _ = app.emit(
            MATRIX_EVENT,
            &MatrixEvent {
                key: "__done".to_string(),
                protocol: String::new(),
                transport: String::new(),
                phase: "finished".to_string(),
                ok: true,
                ms: None,
                exit: None,
                note: format!("matrix finished at {}", now_millis()),
            },
        );
    });
    Ok(())
}

#[tauri::command]
pub fn matrix_cancel() {
    CANCEL.store(true, Ordering::SeqCst);
}

fn run_combo(
    app: &AppHandle,
    manager: &Arc<Mutex<crate::aether::AetherManager>>,
    base: &ConnectionProfile,
    combo: &TestCombo,
) {
    let mut profile = base.clone();
    profile.protocol = combo.protocol.clone();
    profile.extra_transport = combo.transport.clone();
    emit(app, combo, "running", false, None, None, "connecting…".to_string());
    if crate::aether::start_connect(app.clone(), manager.clone(), Some(profile)).is_err() {
        emit(app, combo, "done", false, None, None, "could not start".to_string());
        return;
    }
    let deadline = Instant::now() + Duration::from_secs(combo.budget_secs);
    loop {
        if CANCEL.load(Ordering::SeqCst) {
            let _ = crate::aether::request_disconnect(app, manager);
            emit(app, combo, "done", false, None, None, "cancelled".to_string());
            return;
        }
        match manager.lock().unwrap().status() {
            ConnectionState::Connected { socks_addr, .. } => {
                let socks = socks_addr.clone();
                match measure_latency(&socks) {
                    Some(ms) => emit(app, combo, "done", true, Some(ms), Some(socks), String::new()),
                    None => emit(app, combo, "done", false, None, Some(socks), "connected, no traffic".to_string()),
                }
                let _ = crate::aether::request_disconnect(app, manager);
                return;
            }
            ConnectionState::Error { message, .. } => {
                let _ = crate::aether::request_disconnect(app, manager);
                emit(app, combo, "done", false, None, None, message);
                return;
            }
            ConnectionState::Idle | ConnectionState::Disconnecting => {
                // Stopped from outside (user hit disconnect): fail fast.
                emit(app, combo, "done", false, None, None, "stopped".to_string());
                return;
            }
            _ => {}
        }
        if Instant::now() >= deadline {
            let _ = crate::aether::request_disconnect(app, manager);
            emit(app, combo, "done", false, None, None, "timeout".to_string());
            return;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_has_30_curated_combos() {
        let combos = matrix_combos();
        assert_eq!(combos.len(), 30);
        // No reverses: MASQUE-only by construction, covered by masque rows.
        assert!(!combos.iter().any(|c| matches!(
            c.transport,
            ExtraTransport::TorReverse | ExtraTransport::PsiphonReverse
        )));
        // Budgets match bootstrap reality.
        let tor = combos.iter().find(|c| c.transport == ExtraTransport::Tor).unwrap();
        let plain = combos.iter().find(|c| c.transport == ExtraTransport::None).unwrap();
        assert!(tor.budget_secs > plain.budget_secs);
    }
}
