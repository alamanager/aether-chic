//! Connection matrix lab: try every meaningful protocol × transport
//! combo for real, all at once (one lane per combo).
//!
//! The matrix is curated, not cartesian: reverses are MASQUE-only by
//! construction (covered by the masque rows), scan/noize/ip come from the
//! current profile so transports compare fairly. Fresh scan every combo
//! (quick_reconnect off, ZeroTrust team stripped) so results reflect
//! reachability, not cache or identity.
//!
//! Isolation per lane: own AetherManager, own data dir (own WARP
//! identity + pid file), own ports (SOCKS 183x, Tor exits 184x, Psiphon
//! exits 185x, HTTP = SOCKS+1). Lane sessions never touch the main
//! manager; the frontend additionally ignores global status/log events
//! while a run is active (see setMatrixActive).
use crate::aether::profiles::{ConnectionProfile, ExtraTransport, Protocol};
use crate::error::AetherError;
use crate::events::{now_millis, MATRIX_EVENT};
use crate::state::{AppState, ConnectionState};
use serde::Serialize;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
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

/// The curated matrix: 6 protocols × 3 transports = 18 real connects,
/// ALL at once (one lane per combo). No Tor rows — Tor bootstraps for
/// minutes and never answers "does it connect" quickly; test Tor by hand.
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
        (ExtraTransport::None, 60u64),
        (ExtraTransport::Psiphon, 150u64),
        (ExtraTransport::PsiphonOnly, 150u64),
    ];
    let mut out = Vec::with_capacity(18);
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

/// Distinct loopback ports per lane: SOCKS 183x, Psiphon exits 190x,
/// HTTP = SOCKS+1 (odd, never collides with the even ranges).
fn lane_ports(lane: usize) -> (String, String, String) {
    let socks = 1830 + (lane as u16) * 2;
    (
        format!("127.0.0.1:{socks}"),
        format!("127.0.0.1:{}", 1900 + (lane as u16) * 2),
        (socks + 1).to_string(),
    )
}

static CANCEL: AtomicBool = AtomicBool::new(false);

fn emit(
    app: &AppHandle,
    combo: &TestCombo,
    phase: &str,
    ok: bool,
    ms: Option<u64>,
    exit: Option<String>,
    note: String,
) {
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
    // last time", not "what reaches from here". Team stripped: a matrix
    // run tests transports, never identity (and must never block on an
    // access-code prompt).
    base.quick_reconnect = false;
    base.zero_trust_team = String::new();
    let data_root = crate::aether::app_data_dir(&app);
    let combos = matrix_combos();
    // One lane per combo: everything simultaneously.
    let lanes = combos.len();
    let remaining = Arc::new(AtomicUsize::new(lanes));
    for lane in 0..lanes {
        let app = app.clone();
        let base = base.clone();
        let queue: Vec<TestCombo> = combos
            .iter()
            .enumerate()
            .filter(|(i, _)| i % lanes == lane)
            .map(|(_, c)| TestCombo {
                protocol: c.protocol.clone(),
                transport: c.transport.clone(),
                budget_secs: c.budget_secs,
            })
            .collect();
        let remaining = remaining.clone();
        let data_root = data_root.clone();
        std::thread::spawn(move || {
            let lane_manager = Arc::new(Mutex::new(crate::aether::AetherManager::new()));
            let lane_dir = data_root.join(format!("matrix-slot-{lane}"));
            for combo in &queue {
                if CANCEL.load(Ordering::SeqCst) {
                    break;
                }
                run_combo(&app, &lane_manager, &lane_dir, &base, combo, lane);
                // Cooldown so the next bind never trips on our own TIME_WAIT.
                for _ in 0..2 {
                    if CANCEL.load(Ordering::SeqCst) {
                        break;
                    }
                    std::thread::sleep(Duration::from_secs(1));
                }
            }
            let _ = crate::aether::request_disconnect(&app, &lane_manager);
            if remaining.fetch_sub(1, Ordering::SeqCst) == 1 {
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
            }
        });
    }
    Ok(())
}

#[tauri::command]
pub fn matrix_cancel() {
    CANCEL.store(true, Ordering::SeqCst);
}

fn run_combo(
    app: &AppHandle,
    lane: &Arc<Mutex<crate::aether::AetherManager>>,
    lane_dir: &std::path::PathBuf,
    base: &ConnectionProfile,
    combo: &TestCombo,
    lane_idx: usize,
) {
    let (bind, psi_bind, http_port) = lane_ports(lane_idx);
    let mut profile = base.clone();
    profile.protocol = combo.protocol.clone();
    profile.extra_transport = combo.transport.clone();
    profile.bind_address = bind;
    profile.psiphon_bind = psi_bind;
    profile.http_port = http_port;
    emit(app, combo, "running", false, None, None, "connecting…".to_string());
    if crate::aether::start_connect_in(app.clone(), lane.clone(), lane_dir.clone(), Some(profile))
        .is_err()
    {
        emit(app, combo, "done", false, None, None, "could not start".to_string());
        return;
    }
    let deadline = Instant::now() + Duration::from_secs(combo.budget_secs);
    loop {
        if CANCEL.load(Ordering::SeqCst) {
            let _ = crate::aether::request_disconnect(app, lane);
            emit(app, combo, "done", false, None, None, "cancelled".to_string());
            return;
        }
        match lane.lock().unwrap().status() {
            ConnectionState::Connected { socks_addr, .. } => {
                let socks = socks_addr.clone();
                match measure_latency(&socks) {
                    Some(ms) => emit(app, combo, "done", true, Some(ms), Some(socks), String::new()),
                    None => emit(
                        app,
                        combo,
                        "done",
                        false,
                        None,
                        Some(socks),
                        "connected, no traffic".to_string(),
                    ),
                }
                let _ = crate::aether::request_disconnect(app, lane);
                return;
            }
            ConnectionState::Error { message, .. } => {
                let _ = crate::aether::request_disconnect(app, lane);
                emit(app, combo, "done", false, None, None, message);
                return;
            }
            ConnectionState::Idle | ConnectionState::Disconnecting => {
                // Stopped from outside: fail fast.
                emit(app, combo, "done", false, None, None, "stopped".to_string());
                return;
            }
            _ => {}
        }
        if Instant::now() >= deadline {
            let _ = crate::aether::request_disconnect(app, lane);
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
    fn matrix_has_18_curated_combos() {
        let combos = matrix_combos();
        assert_eq!(combos.len(), 18);
        // No Tor at all (bootstrap answers nothing quickly), no reverses
        // (MASQUE-only, covered by masque rows).
        assert!(!combos.iter().any(|c| matches!(
            c.transport,
            ExtraTransport::Tor
                | ExtraTransport::TorOnly
                | ExtraTransport::TorReverse
                | ExtraTransport::PsiphonReverse
        )));
        // Budgets match bootstrap reality.
        let psi = combos.iter().find(|c| c.transport == ExtraTransport::Psiphon).unwrap();
        let plain = combos.iter().find(|c| c.transport == ExtraTransport::None).unwrap();
        assert!(psi.budget_secs > plain.budget_secs);
    }

    #[test]
    fn lanes_get_distinct_ports() {
        let n = matrix_combos().len();
        let mut seen = std::collections::HashSet::new();
        for lane in 0..n {
            let (bind, psi, http) = lane_ports(lane);
            for p in [bind, psi] {
                assert!(seen.insert(p.clone()), "port reused: {p}");
            }
            // HTTP rides SOCKS+1 of its own lane, odd — never an exit.
            let socks_port: u16 = lane_ports(lane).0.rsplit(':').next().unwrap().parse().unwrap();
            assert_eq!(http, (socks_port + 1).to_string());
        }
    }
}
