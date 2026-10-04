//! Windows system-proxy switch for the tunnel's HTTP endpoint.
//!
//! Mechanism mirrors v2rayN's ProxySettingWindows (the working reference,
//! read in full): a `ProxyServer` value + `ProxyOverride` bypass list under
//! HKCU\...\Internet Settings, activated with WinInet
//! `INTERNET_OPTION_SETTINGS_CHANGED` + `INTERNET_OPTION_REFRESH`.
//!
//! The server handed here is the core's native HTTP proxy, so the value is
//! written bare ("host:port") — Windows treats that as an HTTP proxy for
//! all protocols, which is exactly right for an HTTP endpoint.
//!
//! Unlike v2rayN's fallback (and our first version), disabling RESTORES the
//! user's previous setting instead of wiping it: the prior values are
//! snapshotted to `sysproxy-backup.json` before anything changes. A crash
//! while set leaves the backup behind, and the next launch restores it, so
//! a dead proxy never strands the user without internet.

use std::path::PathBuf;
use std::process::Command;
use tauri::{AppHandle, Manager};

const REG_PATH: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings";
const BACKUP_FILE: &str = "sysproxy-backup.json";

/// The user's pre-existing proxy state. `None` = value was absent and must
/// be deleted (not zeroed) on restore to leave no trace.
#[derive(serde::Serialize, serde::Deserialize)]
struct Backup {
    enable: Option<u32>,
    server: Option<String>,
    bypass: Option<String>,
    autoconfig: Option<String>,
}

fn backup_file(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
        .join(BACKUP_FILE)
}

fn reg(args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("reg");
    // Silent: every spawn of a console binary flashes a CMD window otherwise
    // (this runs 4+ times per proxy toggle — the "ten flashing windows").
    crate::cmd::silent(&mut cmd);
    let out = cmd
        .args(args)
        .output()
        .map_err(|e| format!("failed to run `reg`: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Reads one registry value. Returns None when absent; DWORDs come back
/// as decimal strings ("0"/"1").
fn query_value(name: &str) -> Option<String> {
    let out = reg(&["query", REG_PATH, "/v", name]).ok()?;
    for line in out.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("HKEY") {
            continue;
        }
        let mut parts = line.split_whitespace();
        parts.next()?;
        match parts.next()? {
            "REG_DWORD" => {
                let raw = parts.next()?;
                let n = u32::from_str_radix(raw.trim_start_matches("0x"), 16).ok()?;
                return Some(n.to_string());
            }
            _ => {
                let rest: Vec<&str> = parts.collect();
                return Some(rest.join(" "));
            }
        }
    }
    None
}

fn snapshot() -> Backup {
    Backup {
        enable: query_value("ProxyEnable").and_then(|v| v.parse().ok()),
        server: query_value("ProxyServer"),
        bypass: query_value("ProxyOverride"),
        autoconfig: query_value("AutoConfigURL"),
    }
}

fn write_snapshot(app: &AppHandle) -> Result<(), String> {
    let text = serde_json::to_string(&snapshot()).map_err(|e| e.to_string())?;
    std::fs::write(backup_file(app), text).map_err(|e| e.to_string())
}

/// Puts a snapshot back. Missing values are deleted rather than zeroed so
/// machines that never had a proxy set keep it that way.
fn restore_snapshot(app: &AppHandle) {
    let path = backup_file(app);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return,
    };
    let saved: Backup = match serde_json::from_str(&text) {
        Ok(b) => b,
        Err(_) => return,
    };
    match saved.enable {
        Some(n) => {
            let _ = reg(&["add", REG_PATH, "/v", "ProxyEnable", "/t", "REG_DWORD", "/d", &n.to_string(), "/f"]);
        }
        None => {
            let _ = reg(&["delete", REG_PATH, "/v", "ProxyEnable", "/f"]);
        }
    }
    for (name, val) in [
        ("ProxyServer", saved.server),
        ("ProxyOverride", saved.bypass),
        ("AutoConfigURL", saved.autoconfig),
    ] {
        match val {
            Some(v) => {
                let _ = reg(&["add", REG_PATH, "/v", name, "/t", "REG_SZ", "/d", &v, "/f"]);
            }
            None => {
                let _ = reg(&["delete", REG_PATH, "/v", name, "/f"]);
            }
        }
    }
    let _ = std::fs::remove_file(&path);
    refresh();
}

/// Proxy bypass list: plain hostnames, localhost, and all RFC 1918 private
/// ranges. Windows' "<local>" alone does NOT cover literal LAN IPs, so
/// without these every 192.168.x.x / 10.x.x.x address would be sent into
/// the tunnel (and fail there) instead of going direct on the LAN.
fn proxy_bypass() -> String {
    let mut parts = vec![
        "<local>".to_string(),
        "localhost".to_string(),
        "127.*".to_string(),
        "10.*".to_string(),
        "192.168.*".to_string(),
    ];
    for i in 16..=31 {
        parts.push(format!("172.{i}.*"));
    }
    parts.join(";")
}

#[cfg(windows)]
fn refresh() {
    use windows_sys::Win32::Networking::WinInet::{
        INTERNET_OPTION_REFRESH, INTERNET_OPTION_SETTINGS_CHANGED, InternetSetOptionW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE,
    };
    unsafe {
        // Makes WinInet re-read the registry — without this the new values
        // sit ignored in HKCU until reboot.
        InternetSetOptionW(
            std::ptr::null_mut(),
            INTERNET_OPTION_SETTINGS_CHANGED,
            std::ptr::null_mut(),
            0,
        );
        InternetSetOptionW(
            std::ptr::null_mut(),
            INTERNET_OPTION_REFRESH,
            std::ptr::null_mut(),
            0,
        );
        // ...plus a broadcast on top for non-WinInet consumers.
        let setting: Vec<u16> = "Environment\0".encode_utf16().collect();
        let mut _result: usize = 0;
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            setting.as_ptr() as isize,
            SMTO_ABORTIFHUNG,
            5000,
            &mut _result as *mut usize,
        );
    }
}

pub fn get() -> Result<bool, String> {
    #[cfg(not(windows))]
    {
        return Err("system proxy switch is only supported on Windows".into());
    }
    #[cfg(windows)]
    {
        match reg(&["query", REG_PATH, "/v", "ProxyEnable"]) {
            Ok(stdout) => Ok(stdout.contains("0x1")),
            // Never configured on this machine → effectively off, not an
            // error (otherwise the UI switch would stay disabled forever).
            Err(e) if e.contains("unable to find") => Ok(false),
            Err(e) => Err(e),
        }
    }
}

pub fn set(app: &AppHandle, enabled: bool, server: &str) -> Result<(), String> {
    #[cfg(not(windows))]
    {
        let _ = (app, enabled, server);
        return Err("system proxy switch is only supported on Windows".into());
    }
    #[cfg(windows)]
    {
        if enabled {
            let server = server.trim();
            if server.is_empty() {
                return Err("proxy server address is empty".into());
            }
            let backup = backup_file(app);
            // A backup left by a crashed run holds the user's REAL settings —
            // restore those first and never overwrite them with ours.
            if backup.exists() {
                restore_snapshot(app);
            }
            write_snapshot(app)?;
            // Bare "host:port" = HTTP proxy for all protocols — correct
            // here because `server` is the core's HTTP endpoint, not SOCKS.
            reg(&[
                "add", REG_PATH, "/v", "ProxyServer", "/t", "REG_SZ", "/d", server, "/f",
            ])?;
            reg(&[
                "add",
                REG_PATH,
                "/v",
                "ProxyOverride",
                "/t",
                "REG_SZ",
                "/d",
                proxy_bypass().as_str(),
                "/f",
            ])?;
            // A stale PAC URL would override ProxyServer — make sure it's gone.
            let _ = reg(&["delete", REG_PATH, "/v", "AutoConfigURL", "/f"]);
            reg(&[
                "add", REG_PATH, "/v", "ProxyEnable", "/t", "REG_DWORD", "/d", "1", "/f",
            ])?;
        } else if backup_file(app).exists() {
            restore_snapshot(app);
            return Ok(());
        } else {
            // No snapshot (e.g. proxy was set externally while we ran):
            // fall back to switching it off without touching the rest.
            reg(&[
                "add", REG_PATH, "/v", "ProxyEnable", "/t", "REG_DWORD", "/d", "0", "/f",
            ])?;
        }
        refresh();
        Ok(())
    }
}

/// Startup safety net: undo a proxy a previous (crashed) run left behind,
///
/// so a dead proxy never strands the user without internet.
pub fn restore_stale(app: &AppHandle) {
    #[cfg(windows)]
    {
        if backup_file(app).exists() {
            restore_snapshot(app);
        }
    }
    #[cfg(not(windows))]
    {
        let _ = app;
    }
}
