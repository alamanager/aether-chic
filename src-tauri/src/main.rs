#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod aether;
mod clients;
mod cmd;
mod commands;
mod error;
mod events;
mod focus;
mod state;
mod sysproxy;
mod tray;

use state::AppState;
use tauri::{Manager, WindowEvent};

fn main() {
    // Elevated-takeover handshake (see request_admin): the relaunch passes
    // this flag so the parent knows the elevated copy booted. Touch the
    // file and boot normally — no auto-connect, so no port fight.
    for arg in std::env::args().skip(1) {
        if let Some(path) = arg.strip_prefix("--elevated-ready=") {
            let _ = std::fs::write(path, "ready");
        }
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .manage(AppState::default())
        .setup(|app| {
            let data_dir = app.handle().path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            // Reap any Aether process left running from a prior crash before
            // the user can click Connect and spawn a second one onto the
            // same port.
            aether::orphan::reap_orphan(&data_dir);
            // Undo a system proxy a crashed run left behind, so a dead
            // proxy never strands the user without internet.
            crate::sysproxy::restore_stale(&app.handle());
            focus::spawn_watcher(app.handle().clone());
            tray::init(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::connect,
            commands::disconnect,
            commands::submit_access_code,
            commands::get_status,
            commands::get_default_profile,
            commands::set_default_profile,
            commands::get_close_to_tray,
            commands::set_close_to_tray,
            commands::get_system_proxy,
            commands::set_system_proxy,
            clients::proxy_clients,
            clients::resolve_host,
            clients::is_elevated,
            clients::request_admin,
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if tray::get_close_to_tray() {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                let state = app_handle.state::<AppState>();
                let data_dir = app_handle
                    .path()
                    .app_data_dir()
                    .unwrap_or_else(|_| std::env::temp_dir());
                aether::shutdown_blocking(&state.manager, &data_dir);
            }
        });
}
