//! Windows system-proxy switch for the tunnel's HTTP endpoint.
//!
//! Mechanism mirrors v2rayN's ProxySettingWindows (the working reference,
//! read in full): a `ProxyServer` value + `ProxyOverride` bypass list under
//! HKCU\...\Internet Settings, activated with WinInet
//! `INTERNET_OPTION_SETTINGS_CHANGED` + `INTERNET_OPTION_REFRESH`.
//!
//! The server handed here is the local HTTP→SOCKS bridge (see http_proxy),
//! so the value is written bare ("host:port") — Windows treats that as an
//! HTTP proxy for all protocols, which is exactly right for an HTTP
//! endpoint. (A SOCKS-only endpoint would need a "socks=" prefix instead;
//! that's why the bare form failed before the bridge existed.)
//! ponytail: disabling wipes ProxyServer/Override instead of restoring
//! whatever was there before — same as v2rayN's fallback. Snapshot/restore
//! only if a user ever complains.

use std::process::Command;

const REG_PATH: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings";

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

pub fn set(enabled: bool, server: &str) -> Result<(), String> {
    #[cfg(not(windows))]
    {
        let _ = (enabled, server);
        return Err("system proxy switch is only supported on Windows".into());
    }
    #[cfg(windows)]
    {
        if enabled {
            let server = server.trim();
            if server.is_empty() {
                return Err("proxy server address is empty".into());
            }
            // Bare "host:port" = HTTP proxy for all protocols — correct
            // here because `server` is the HTTP bridge, not raw SOCKS.
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
        } else {
            reg(&[
                "add", REG_PATH, "/v", "ProxyEnable", "/t", "REG_DWORD", "/d", "0", "/f",
            ])?;
            reg(&[
                "add", REG_PATH, "/v", "ProxyServer", "/t", "REG_SZ", "/d", "", "/f",
            ])?;
            reg(&[
                "add", REG_PATH, "/v", "ProxyOverride", "/t", "REG_SZ", "/d", "", "/f",
            ])?;
        }
        refresh();
        Ok(())
    }
}
