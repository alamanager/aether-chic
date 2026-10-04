use crate::aether::{self, profiles::ConnectionProfile};
use crate::error::AetherError;
use crate::state::{AppState, ConnectionState};
use crate::tray;
use tauri::{AppHandle, State};

#[tauri::command]
pub fn connect(
    app: AppHandle,
    state: State<AppState>,
    profile_override: Option<ConnectionProfile>,
) -> Result<(), AetherError> {
    // Load once so both branches decide on the same profile.
    let profile = profile_override.unwrap_or_else(|| aether::profiles::load(&app));
    if profile.extra_transport.is_direct() {
        // Direct console-client mode bypasses the core entirely.
        let busy = !matches!(
            state.manager.lock().unwrap().status(),
            ConnectionState::Idle | ConnectionState::Error { .. }
        );
        if busy {
            return Err(AetherError::AlreadyRunning);
        }
        return crate::psiphon_direct::start(app, profile);
    }
    if crate::psiphon_direct::is_active() {
        return Err(AetherError::AlreadyRunning);
    }
    aether::start_connect(app, state.manager.clone(), Some(profile))
}

#[tauri::command]
pub fn disconnect(app: AppHandle, state: State<AppState>) -> Result<(), AetherError> {
    crate::psiphon_direct::stop(&app);
    aether::request_disconnect(&app, &state.manager)
}

#[tauri::command]
pub fn submit_access_code(state: State<AppState>, code: String) -> Result<(), AetherError> {
    aether::submit_access_code(&state.manager, code)
}

#[tauri::command]
pub fn get_status(state: State<AppState>) -> ConnectionState {
    if crate::psiphon_direct::is_active() {
        return crate::psiphon_direct::status();
    }
    state.manager.lock().unwrap().status()
}

#[tauri::command]
pub fn get_default_profile(app: AppHandle) -> ConnectionProfile {
    aether::profiles::load(&app)
}

#[tauri::command]
pub fn set_default_profile(app: AppHandle, profile: ConnectionProfile) -> Result<(), AetherError> {
    aether::profiles::save(&app, &profile);
    Ok(())
}

#[tauri::command]
pub fn get_close_to_tray() -> bool {
    tray::get_close_to_tray()
}

#[tauri::command]
pub fn set_close_to_tray(app: AppHandle, enabled: bool) {
    tray::set_close_to_tray(&app, enabled);
}

#[tauri::command]
pub fn get_system_proxy() -> Result<bool, AetherError> {
    crate::sysproxy::get().map_err(AetherError::Internal)
}

#[tauri::command]
pub fn set_system_proxy(app: AppHandle, enabled: bool, server: String) -> Result<(), AetherError> {
    crate::sysproxy::set(&app, enabled, &server).map_err(AetherError::Internal)
}
