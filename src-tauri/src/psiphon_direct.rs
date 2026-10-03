//! Direct psiphon-tunnel-core mode: drives the bundled console client
//! (`binaries/pt/psiphon-tunnel-core`, built from CluvexStudio's fork)
//! WITHOUT going through the Aether core — an alternative Psiphon path with
//! the same public credentials Aether itself uses (IDs F…F, S3 server list).
//!
//! Fully additive: the Aether-core flows in aether/mod.rs are untouched.
//! State is reported with the same ConnectionState events, so the UI needs
//! no new states. Fixed ports (SOCKS 11819 / HTTP 11820) keep it separable
//! from the core's 1819/1820.

use crate::aether::profiles::ConnectionProfile;
use crate::error::AetherError;
use crate::events::{now_millis, LogEvent, LOG_EVENT, STATUS_EVENT};
use crate::state::ConnectionState;
use std::io::{BufRead, BufReader};
use std::net::SocketAddr;
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

pub const DIRECT_SOCKS: &str = "127.0.0.1:11819";
pub const DIRECT_HTTP: &str = "127.0.0.1:11820";

const PROPAGATION_CHANNEL_ID: &str = "FFFFFFFFFFFFFFFF";
const SPONSOR_ID: &str = "FFFFFFFFFFFFFFFF";
const SERVER_LIST_URL: &str =
    "https://s3.amazonaws.com//psiphon/web/mjr4-p23r-puwl/server_list_compressed";
// Same public key Aether's own psiphon.rs uses (concat of its chunks).
const SERVER_LIST_SIGNATURE_KEY: &str = concat!(
    "MIICIDANBgkqhkiG9w0BAQEFAAOCAg0AMIICCAKCAgEAt7Ls+/39r+T6zNW7GiVpJfzq/xvL9SBH",
    "5rIFnk0RXYEYavax3WS6HOD35eTAqn8AniOwiH+DOkvgSKF2caqk/y1dfq47Pdymtwzp9ikpB1C5",
    "OfAysXzBiwVJlCdajBKvBZDerV1cMvRzCKvKwRmvDmHgphQQ7WfXIGbRbmmk6opMBh3roE42Kcot",
    "LFtqp0RRwLtcBRNtCdsrVsjiI1Lqz/lH+T61sGjSjQ3CHMuZYSQJZo/KrvzgQXpkaCTdbObxHqb6",
    "/+i1qaVOfEsvjoiyzTxJADvSytVtcTjijhPEV6XskJVHE1Zgl+7rATr/pDQkw6DPCNBS1+Y6fy7G",
    "stZALQXwEDN/qhQI9kWkHijT8ns+i1vGg00Mk/6J75arLhqcodWsdeG/M/moWgqQAnlZAGVtJI1O",
    "geF5fsPpXu4kctOfuZlGjVZXQNW34aOzm8r8S0eVZitPlbhcPiR4gT/aSMz/wd8lZlzZYsje/Jr8",
    "u/YtlwjjreZrGRmG8KMOzukV3lLmMppXFMvl4bxv6YFEmIuTsOhbLTwFgh7KYNjodLj/LsqRVfwz",
    "31PgWQFTEPICV7GCvgVlPRxnofqKSjgTWI4mxDhBpVcATvaoBl1L/6WLbFvBsoAUBItWwctO2xal",
    "KxF5szhGm8lccoc5MZr8kfE0uxMgsxz4er68iCID+rsCAQM=",
);
const CDN_PROTOCOLS: &[&str] = &[
    "FRONTED-MEEK-CDN-OSSH",
    "FRONTED-MEEK-CDN-HTTP-OSSH",
    "FRONTED-MEEK-CDN-QUIC-OSSH",
];
const DIRECT_PROTOCOLS: &[&str] = &[
    "SSH",
    "OSSH",
    "TLS-OSSH",
    "UNFRONTED-MEEK-OSSH",
    "UNFRONTED-MEEK-HTTPS-OSSH",
    "UNFRONTED-MEEK-SESSION-TICKET-OSSH",
    "QUIC-OSSH",
    "SHADOWSOCKS-OSSH",
];

struct DirectState {
    child: Option<Child>,
    status: ConnectionState,
}

fn cell() -> &'static Mutex<DirectState> {
    static CELL: OnceLock<Mutex<DirectState>> = OnceLock::new();
    CELL.get_or_init(|| {
        Mutex::new(DirectState {
            child: None,
            status: ConnectionState::Idle,
        })
    })
}

pub fn is_active() -> bool {
    let st = cell().lock().unwrap();
    !matches!(st.status, ConnectionState::Idle | ConnectionState::Error { .. })
}

pub fn status() -> ConnectionState {
    cell().lock().unwrap().status.clone()
}

fn set_status(app: &AppHandle, status: ConnectionState) {
    cell().lock().unwrap().status = status.clone();
    let _ = app.emit(STATUS_EVENT, &status);
}

fn app_data_dir(app: &AppHandle) -> std::path::PathBuf {
    crate::aether::app_data_dir(app)
}

fn binary_path(app: &AppHandle) -> Result<std::path::PathBuf, AetherError> {
    let dir = app
        .path()
        .resource_dir()
        .map_err(|e| AetherError::Internal(e.to_string()))?;
    let name = if cfg!(windows) {
        "psiphon-tunnel-core.exe"
    } else {
        "psiphon-tunnel-core"
    };
    let path = dir.join("binaries").join("pt").join(name);
    if !path.exists() {
        return Err(AetherError::BinaryMissing(path.display().to_string()));
    }
    Ok(path)
}

fn write_config(
    dir: &std::path::Path,
    profile: &ConnectionProfile,
) -> Result<std::path::PathBuf, AetherError> {
    let mut map = serde_json::Map::new();
    map.insert(
        "PropagationChannelId".into(),
        serde_json::Value::from(PROPAGATION_CHANNEL_ID),
    );
    map.insert("SponsorId".into(), serde_json::Value::from(SPONSOR_ID));
    map.insert(
        "RemoteServerListURLs".into(),
        serde_json::Value::from(vec![SERVER_LIST_URL]),
    );
    map.insert(
        "RemoteServerListSignaturePublicKey".into(),
        serde_json::Value::from(SERVER_LIST_SIGNATURE_KEY),
    );
    map.insert(
        "LocalSocksProxyPort".into(),
        serde_json::Value::from(11819),
    );
    map.insert(
        "LocalHttpProxyPort".into(),
        serde_json::Value::from(11820),
    );
    let region = profile.psiphon_region.trim().to_uppercase();
    if !region.is_empty() {
        map.insert("EgressRegion".into(), serde_json::Value::from(region));
    }
    // Same shape mapping Aether's own psiphon.rs uses.
    match profile.psiphon_mode.as_str() {
        "cdn" => {
            map.insert(
                "LimitTunnelProtocols".into(),
                serde_json::Value::from(
                    CDN_PROTOCOLS.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                ),
            );
        }
        "direct" => {
            map.insert(
                "LimitTunnelProtocols".into(),
                serde_json::Value::from(
                    DIRECT_PROTOCOLS
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>(),
                ),
            );
        }
        _ => {}
    }
    let path = dir.join("psiphon-direct-config.json");
    std::fs::write(&path, serde_json::Value::from(map).to_string())
        .map_err(|e| AetherError::Internal(format!("psiphon config {path:?}: {e}")))?;
    Ok(path)
}

/// Starts the direct console client. Fails fast when another session (core
/// or direct) owns the ports.
pub fn start(
    app: AppHandle,
    profile: ConnectionProfile,
) -> Result<(), AetherError> {
    {
        let st = cell().lock().unwrap();
        if !matches!(
            st.status,
            ConnectionState::Idle | ConnectionState::Error { .. }
        ) {
            return Err(AetherError::AlreadyRunning);
        }
    }
    let socks: SocketAddr = DIRECT_SOCKS.parse().expect("literal parses");
    let http: SocketAddr = DIRECT_HTTP.parse().expect("literal parses");
    if crate::aether::status::port_is_live(&socks)
        || crate::aether::status::port_is_live(&http)
    {
        return Err(AetherError::PortInUse(11819));
    }

    let binary = binary_path(&app)?;
    let data_dir = app_data_dir(&app).join("psiphon-direct");
    std::fs::create_dir_all(&data_dir).map_err(|e| AetherError::Internal(e.to_string()))?;
    let config = write_config(&data_dir, &profile)?;

    set_status(&app, ConnectionState::Launching);

    let mut cmd = Command::new(&binary);
    crate::cmd::silent(&mut cmd);
    cmd.arg("-config")
        .arg(&config)
        .arg("-dataRootDirectory")
        .arg(&data_dir)
        .arg("-listenInterface")
        .arg("127.0.0.1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(&data_dir);
    let mut child = cmd
        .spawn()
        .map_err(|e| AetherError::SpawnFailed(format!("psiphon-tunnel-core: {e}")))?;

    // Forward stderr lines to the log panel (same UX as the core path).
    if let Some(err) = child.stderr.take() {
        let app_for_logs = app.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(err).lines().map_while(Result::ok) {
                let _ = app_for_logs.emit(
                    LOG_EVENT,
                    &LogEvent {
                        line,
                        timestamp: now_millis(),
                    },
                );
            }
        });
    }
    // Drain stdout so a chatty child can never block on a full pipe.
    if let Some(out) = child.stdout.take() {
        std::thread::spawn(move || {
            for _ in BufReader::new(out).lines().map_while(Result::ok) {}
        });
    }

    cell().lock().unwrap().child = Some(child);
    set_status(&app, ConnectionState::Connecting);

    std::thread::spawn(move || monitor(app, http, socks));
    Ok(())
}

fn monitor(app: AppHandle, http: SocketAddr, socks: SocketAddr) {
    let deadline = Instant::now() + crate::aether::status::EXTRA_TRANSPORT_TIMEOUT;
    // Phase 1: local listeners must appear (fast; absence means the child
    // died — surfaced below via try_wait in the wait loop).
    let up_by = Instant::now() + Duration::from_secs(60);
    loop {
        if crate::aether::status::port_is_live(&socks) {
            break;
        }
        if child_exited() || Instant::now() >= up_by {
            return fail(
                app,
                if child_exited() {
                    "psiphon-tunnel-core exited before listening"
                } else {
                    "psiphon-tunnel-core never opened its ports"
                },
            );
        }
        std::thread::sleep(Duration::from_millis(400));
    }
    // Phase 2: real traffic must flow (same bar as the core path).
    if !crate::aether::status::wait_until_usable(&http, deadline) {
        return fail(app, "Timed out waiting for Psiphon direct to become usable");
    }
    set_status(
        &app,
        ConnectionState::Connected {
            socks_addr: DIRECT_SOCKS.into(),
            connected_at_ms: now_millis(),
        },
    );
}

fn child_exited() -> bool {
    matches!(
        cell()
            .lock()
            .unwrap()
            .child
            .as_mut()
            .map(|c| c.try_wait()),
        Some(Ok(Some(_)))
    )
}

fn fail(app: AppHandle, message: &str) {
    stop_child();
    set_status(
        &app,
        ConnectionState::Error {
            message: message.into(),
            phase: "connecting".into(),
        },
    );
}

/// Kills the child if running and returns to Idle (emitted, so the UI
/// follows). Only touches state this module owns.
pub fn stop(app: &AppHandle) {
    let was_active = is_active();
    stop_child();
    if was_active {
        set_status(app, ConnectionState::Idle);
    }
}

/// For app shutdown: kill without emitting (nobody is left to receive,
/// same contract as aether::shutdown_blocking).
pub fn stop_silent() {
    stop_child();
    cell().lock().unwrap().status = ConnectionState::Idle;
}

fn stop_child() {
    let child = cell().lock().unwrap().child.take();
    if let Some(mut c) = child {
        let _ = c.kill();
        let _ = c.wait();
    }
}
