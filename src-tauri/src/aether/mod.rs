pub mod orphan;
pub mod profiles;
pub mod prompts;
pub mod pty;
pub mod status;

use crate::error::AetherError;
use crate::events::{now_millis, LogEvent, LOG_EVENT, STATUS_EVENT};
use crate::state::ConnectionState;
use profiles::{ConnectionProfile, ExtraTransport};
use pty::PtySession;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

pub struct AetherManager {
    session: Option<PtySession>,
    state: ConnectionState,
    user_requested_stop: bool,
    /// Consecutive auto-retry attempts for the current connection lineage.
    /// Reset to 0 on a fresh user-initiated connect, on reaching Connected
    /// (a proven-working connection earns a full retry budget for whatever
    /// drops it next), and on a user-requested disconnect.
    retry_count: u32,
}

impl AetherManager {
    pub fn new() -> Self {
        Self {
            session: None,
            state: ConnectionState::Idle,
            user_requested_stop: false,
            retry_count: 0,
        }
    }

    pub fn status(&self) -> ConnectionState {
        self.state.clone()
    }
}

pub(crate) fn app_data_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
}

fn resolve_binary(app: &AppHandle) -> Result<PathBuf, AetherError> {
    let dir = app
        .path()
        .resource_dir()
        .map_err(|e| AetherError::Internal(e.to_string()))?;
    let name = if cfg!(windows) {
        "aether.exe"
    } else {
        "aether"
    };
    let path = dir.join("binaries").join(name);
    if !path.exists() {
        return Err(AetherError::BinaryMissing(path.display().to_string()));
    }
    // Bundlers don't reliably preserve the exec bit on resource files, and a
    // non-executable core binary would fail every spawn with a cryptic error.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755));
    }
    Ok(path)
}

fn set_state_and_emit(
    app: &AppHandle,
    manager: &Arc<Mutex<AetherManager>>,
    new_state: ConnectionState,
) {
    manager.lock().unwrap().state = new_state.clone();
    let _ = app.emit(STATUS_EVENT, &new_state);
}

/// Kicks off a connection attempt and returns as soon as Aether is spawned
/// (or a synchronous precondition fails: already running / port already
/// bound / binary missing). The actual Launching -> Connecting -> Connected
/// transitions happen on a background thread and reach the frontend via the
/// `aether://status` event, matching the IPC contract in the approved plan.
pub fn start_connect(
    app: AppHandle,
    manager: Arc<Mutex<AetherManager>>,
    profile_override: Option<ConnectionProfile>,
) -> Result<(), AetherError> {
    // Resolve everything fallible that doesn't touch AetherManager's state
    // first, so that once we transition to Launching below, the only
    // remaining failure mode is pty::spawn itself — which is handled
    // explicitly inside spawn_and_monitor rather than ever leaving the
    // state machine stuck in Launching with no process behind it.
    let profile = profile_override.unwrap_or_else(|| profiles::load(&app));
    let binary = resolve_binary(&app)?;
    let data_dir = app_data_dir(&app);
    std::fs::create_dir_all(&data_dir).map_err(|e| AetherError::Internal(e.to_string()))?;

    {
        let mut mgr = manager.lock().unwrap();
        if !matches!(
            mgr.state,
            ConnectionState::Idle | ConnectionState::Error { .. }
        ) {
            return Err(AetherError::AlreadyRunning);
        }
        // Defensive guard independent of the pid-file mechanism in orphan.rs
        // (covers a manually-started Aether or a missing/corrupted pid file),
        // checked under the same lock as the state check above so a rapid
        // double-click can't race two connect() calls past this guard before
        // the first transitions to Launching.
        let socks = status::parse_bind_address(&profile.bind_address);
        if status::port_is_live(&socks) {
            return Err(AetherError::PortInUse(socks.port()));
        }
        // The core also binds its HTTP proxy (SOCKS port + 1, always passed
        // since the v2.1.0 pin). A stale occupant there would leave SOCKS
        // working while the usability probe — and the system proxy — talk
        // to a dead port, i.e. "connected but nothing works".
        let http = profiles::http_proxy_socket_for(&profile);
        if status::port_is_live(&http) {
            return Err(AetherError::PortInUse(http.port()));
        }
        mgr.state = ConnectionState::Launching;
        // A fresh user-initiated connect always gets a full retry budget,
        // independent of whatever happened on a previous, unrelated attempt.
        mgr.retry_count = 0;
    }
    let _ = app.emit(STATUS_EVENT, &ConnectionState::Launching);

    spawn_and_monitor(app, manager, binary, data_dir, profile)
}

/// Spawns the PTY session and the log-forwarding + monitor threads. Shared
/// by the initial user-initiated connect and by `handle_unexpected_failure`'s
/// auto-retry — both start from the same place (a fresh PTY, `Launching`
/// already set by the caller) and only differ in what led here.
fn spawn_and_monitor(
    app: AppHandle,
    manager: Arc<Mutex<AetherManager>>,
    binary: PathBuf,
    data_dir: PathBuf,
    profile: ConnectionProfile,
) -> Result<(), AetherError> {
    let (log_tx, log_rx) = mpsc::channel::<LogEvent>();
    let session_or_err = pty::spawn(&binary, &data_dir, profile.clone(), log_tx);
    let session = match session_or_err {
        Ok(session) => session,
        Err(e) => {
            // Must not leave the state machine stuck in Launching with no
            // process behind it. A spawn failure is an OS/environment-level
            // problem (not a network drop), so it is not auto-retried —
            // retrying blindly here would just mask a real setup issue.
            set_state_and_emit(
                &app,
                &manager,
                ConnectionState::Error {
                    message: e.to_string(),
                    phase: "launching".into(),
                },
            );
            return Err(e);
        }
    };
    orphan::write_pid(&data_dir, session.pid());

    {
        let mut mgr = manager.lock().unwrap();
        mgr.session = Some(session);
        mgr.user_requested_stop = false;
    }

    // Forward every log line to the frontend's advanced/log panel as it
    // arrives, independent of whether status classification succeeds.
    {
        let app_for_logs = app.clone();
        std::thread::spawn(move || {
            for log in log_rx {
                let _ = app_for_logs.emit(LOG_EVENT, &log);
            }
        });
    }

    {
        let app = app.clone();
        let manager = Arc::clone(&manager);
        let binary = binary.clone();
        let data_dir = data_dir.clone();
        std::thread::spawn(move || monitor_connect(app, manager, binary, data_dir, profile));
    }

    Ok(())
}

/// Common landing spot for every unexpected failure (process exit before
/// connecting, scan timeout, or process exit after being connected) that
/// was NOT a user-requested disconnect. Retries with backoff up to
/// `status::MAX_AUTO_RETRIES` before giving up with a real `Error` — this
/// is what turns a mid-session drop (the "stops all of a sudden" case,
/// worst on gool since it's two nested tunnels, but not exclusive to it)
/// into a brief, visible "Reconnecting" instead of dumping the user back to
/// Idle every time.
fn handle_unexpected_failure(
    app: AppHandle,
    manager: Arc<Mutex<AetherManager>>,
    binary: PathBuf,
    data_dir: PathBuf,
    profile: ConnectionProfile,
    failure_message: String,
    phase: &'static str,
) {
    let attempt = {
        let mut mgr = manager.lock().unwrap();
        if mgr.user_requested_stop {
            // request_disconnect is already handling this exit; don't race
            // it with a retry or an Error state it didn't ask for.
            return;
        }
        mgr.session = None;
        mgr.retry_count += 1;
        mgr.retry_count
    };
    orphan::clear_pid(&data_dir);


    if attempt > status::MAX_AUTO_RETRIES {
        set_state_and_emit(
            &app,
            &manager,
            ConnectionState::Error {
                message: format!(
                    "{failure_message} (gave up after {} retries)",
                    status::MAX_AUTO_RETRIES
                ),
                phase: phase.into(),
            },
        );
        return;
    }

    set_state_and_emit(
        &app,
        &manager,
        ConnectionState::Reconnecting {
            attempt,
            max_attempts: status::MAX_AUTO_RETRIES,
        },
    );

    let backoff = status::RETRY_BACKOFF[(attempt - 1) as usize];
    std::thread::spawn(move || {
        std::thread::sleep(backoff);
        {
            let mgr = manager.lock().unwrap();
            if mgr.user_requested_stop {
                return;
            }
        }
        set_state_and_emit(&app, &manager, ConnectionState::Launching);
        // spawn_and_monitor already lands its own failure in Error/retry —
        // nothing further to do with its Result here.
        let _ = spawn_and_monitor(app, manager, binary, data_dir, profile);
    });
}

fn monitor_connect(
    app: AppHandle,
    manager: Arc<Mutex<AetherManager>>,
    binary: PathBuf,
    data_dir: PathBuf,
    profile: ConnectionProfile,
) {
    let deadline = Instant::now() + status::connect_timeout(&profile.scan_mode);
    let socks = status::parse_bind_address(&profile.bind_address);
    let mut announced_connecting = false;

    loop {
        std::thread::sleep(Duration::from_millis(400));
        let mut mgr = manager.lock().unwrap();
        if mgr.user_requested_stop {
            return;
        }

        if let Some(exit) = mgr.session.as_mut().and_then(|s| s.try_wait()) {
            mgr.session = None;
            drop(mgr);
            handle_unexpected_failure(
                app,
                manager,
                binary,
                data_dir,
                profile,
                format!("Aether exited before connecting ({exit})"),
                "connecting",
            );
            return;
        }

        if !announced_connecting {
            let done = mgr
                .session
                .as_ref()
                .map(|s| s.prompts_done())
                .unwrap_or(false);
            if done {
                mgr.state = ConnectionState::Connecting;
                let new_state = mgr.state.clone();
                drop(mgr);
                let _ = app.emit(STATUS_EVENT, &new_state);
                announced_connecting = true;
                continue;
            }
        }

        if status::port_is_live(&socks) {
            // Tor/Psiphon modes bind the local ports long before the exit is
            // usable (observed: Tor stuck at 15% fetching consensus, Psiphon
            // ~6s behind). Confirm real traffic flows before calling it
            // Connected — the probe sleeps, so the lock must go first.
            let http = profiles::http_proxy_socket_for(&profile);
            let needs_probe = profile.extra_transport != ExtraTransport::None;
            drop(mgr);
            if needs_probe
                && !status::wait_until_usable(
                    &http,
                    Instant::now() + status::EXTRA_TRANSPORT_TIMEOUT,
                )
            {
                let mut mgr = manager.lock().unwrap();
                if mgr.user_requested_stop {
                    return;
                }
                if let Some(session) = mgr.session.as_mut() {
                    session.kill();
                }
                mgr.session = None;
                drop(mgr);
                handle_unexpected_failure(
                    app,
                    manager,
                    binary,
                    data_dir,
                    profile,
                    "Timed out waiting for Tor/Psiphon to become usable".into(),
                    "connecting",
                );
                return;
            }
            let mut mgr = manager.lock().unwrap();
            if mgr.user_requested_stop {
                return;
            }
            if let Some(exit) = mgr.session.as_mut().and_then(|s| s.try_wait()) {
                mgr.session = None;
                drop(mgr);
                handle_unexpected_failure(
                    app,
                    manager,
                    binary,
                    data_dir,
                    profile,
                    format!("Aether exited before connecting ({exit})"),
                    "connecting",
                );
                return;
            }
            let new_state = ConnectionState::Connected {
                // Report the connectable address: a 0.0.0.0 bind is real for
                // the core but useless to clients (see status::client_addr).
                socks_addr: status::client_addr(&socks).to_string(),
                connected_at_ms: now_millis(),
            };
            mgr.state = new_state.clone();
            // Proven working — a future drop earns a fresh full retry budget
            // rather than inheriting whatever it took to get here.
            mgr.retry_count = 0;
            // The core serves its native HTTP proxy on SOCKS port + 1 (see
            // profiles::http_proxy_addr) — the frontend computes the same
            // address locally, so nothing needs remembering here.
            let http_str = http.to_string();
            drop(mgr);
            let _ = app.emit(
                LOG_EVENT,
                &LogEvent {
                    line: format!("[gui] HTTP proxy on {http_str} (SOCKS {socks})"),
                    timestamp: now_millis(),
                },
            );
            let _ = app.emit(STATUS_EVENT, &new_state);
            // Only persisted as "last successful" once actually proven to
            // work, never on a mere attempt (see profiles::save's doc-comment).
            profiles::save(&app, &profile);
            monitor_connected(app, manager, binary, data_dir, profile);
            return;
        }

        if Instant::now() >= deadline {
            if let Some(session) = mgr.session.as_mut() {
                session.kill();
            }
            mgr.session = None;
            drop(mgr);
            handle_unexpected_failure(
                app,
                manager,
                binary,
                data_dir,
                profile,
                "Timed out waiting for Aether to find a working route".into(),
                "connecting",
            );
            return;
        }
    }
}

/// Watches an established connection for an unexpected process exit AND for
/// a silently stalled tunnel: the port can stay open while nothing flows
/// (observed in the field as "Connected but no internet"), and only a real
/// fetch proves otherwise. Three failed heartbeats in a row feed the same
/// auto-retry path as a process exit, so a dead tunnel heals itself instead
/// of sitting green-but-dead.
fn monitor_connected(
    app: AppHandle,
    manager: Arc<Mutex<AetherManager>>,
    binary: PathBuf,
    data_dir: PathBuf,
    profile: ConnectionProfile,
) {
    let http = profiles::http_proxy_socket_for(&profile);
    let mut ticks: u32 = 0;
    let mut dead: u32 = 0;
    loop {
        std::thread::sleep(Duration::from_millis(500));
        let mut mgr = manager.lock().unwrap();
        if mgr.user_requested_stop {
            return;
        }
        if let Some(exit) = mgr.session.as_mut().and_then(|s| s.try_wait()) {
            mgr.session = None;
            drop(mgr);
            handle_unexpected_failure(
                app,
                manager,
                binary,
                data_dir,
                profile,
                format!("Lost connection unexpectedly ({exit})"),
                "connected",
            );
            return;
        }
        // Heartbeat every ~15s. The fetch runs WITHOUT the lock: a dead
        // tunnel can stall the read up to its timeout, and disconnect()
        // needs that same lock to stop us.
        ticks += 1;
        let check = ticks % 30 == 0;
        drop(mgr);
        if !check {
            continue;
        }
        let alive = status::traffic_flows(&http);
        let mut mgr = manager.lock().unwrap();
        if mgr.user_requested_stop {
            return;
        }
        if alive {
            dead = 0;
            continue;
        }
        dead += 1;
        if dead >= 3 {
            if let Some(session) = mgr.session.as_mut() {
                session.kill();
            }
            mgr.session = None;
            drop(mgr);
            handle_unexpected_failure(
                app,
                manager,
                binary,
                data_dir,
                profile,
                "Tunnel stopped carrying traffic".into(),
                "connected",
            );
            return;
        }
    }
}

pub fn request_disconnect(
    app: &AppHandle,
    manager: &Arc<Mutex<AetherManager>>,
) -> Result<(), AetherError> {
    let had_session = {
        let mut mgr = manager.lock().unwrap();
        // Reconnecting has no live session (the old one already exited; the
        // retry's replacement hasn't spawned yet) — still a valid thing to
        // cancel, it just means there's nothing to send Ctrl-C to.
        let reconnecting = matches!(mgr.state, ConnectionState::Reconnecting { .. });
        if mgr.session.is_none() && !reconnecting {
            return Err(AetherError::NotConnected);
        }
        mgr.user_requested_stop = true;
        mgr.retry_count = 0;
        if let Some(session) = mgr.session.as_ref() {
            session.send_ctrl_c();
        }
        mgr.session.is_some()
    };

    if !had_session {
        // Mid-backoff: the retry thread checks user_requested_stop (just set
        // above) before respawning, so setting the flag is enough — there is
        // no process to wait on, so reflect Idle immediately.
    
        set_state_and_emit(app, manager, ConnectionState::Idle);
        return Ok(());
    }

    set_state_and_emit(app, manager, ConnectionState::Disconnecting);

    let app = app.clone();
    let manager = Arc::clone(manager);
    std::thread::spawn(move || {
        let deadline = Instant::now() + status::GRACEFUL_SHUTDOWN_GRACE;
        loop {
            std::thread::sleep(Duration::from_millis(200));
            let mut mgr = manager.lock().unwrap();
            let exited = mgr.session.as_mut().and_then(|s| s.try_wait()).is_some();
            if exited || Instant::now() >= deadline {
                if !exited {
                    if let Some(session) = mgr.session.as_mut() {
                        session.kill();
                    }
                }
                mgr.session = None;
                mgr.user_requested_stop = false;
                drop(mgr);
                orphan::clear_pid(&app_data_dir(&app));
            
                set_state_and_emit(&app, &manager, ConnectionState::Idle);
                return;
            }
        }
    });

    Ok(())
}

/// Supplies the Cloudflare Access one-time code requested by Aether 1.5.0
/// during a Zero Trust email enrolment. It is deliberately a narrow command
/// instead of a generic PTY write endpoint, so the webview can never inject
/// arbitrary terminal input into the bundled core.
pub fn submit_access_code(
    manager: &Arc<Mutex<AetherManager>>,
    code: String,
) -> Result<(), AetherError> {
    let manager = manager
        .lock()
        .map_err(|_| AetherError::Internal("Aether state is unavailable".into()))?;
    let session = manager.session.as_ref().ok_or(AetherError::NotConnected)?;
    session.send_access_code(&code)
}

/// Called from `RunEvent::Exit` — the app is quitting regardless, so this
/// blocks briefly rather than spawning a thread, and skips emitting events
/// nobody is left to receive.
pub fn shutdown_blocking(manager: &Arc<Mutex<AetherManager>>, data_dir: &Path) {
    crate::psiphon_direct::stop_silent();

    let mut mgr = manager.lock().unwrap();
    if let Some(session) = mgr.session.as_mut() {
        session.send_ctrl_c();
        std::thread::sleep(Duration::from_millis(500));
        session.kill();
    }
    mgr.session = None;
    drop(mgr);
    orphan::clear_pid(data_dir);
}