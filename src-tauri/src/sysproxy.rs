//! Windows system-proxy switch for the tunnel's SOCKS5 endpoint.
//!
//! Mechanism mirrors v2rayN's ProxySettingWindows (the working reference,
//! read in full): a per-protocol `ProxyServer` value + `ProxyOverride`
//! bypass list under HKCU\...\Internet Settings, activated with WinInet
//! `INTERNET_OPTION_SETTINGS_CHANGED` + `INTERNET_OPTION_REFRESH`.
//!
//! Two details that broke the first version of this module:
//!  1. A bare "host:port" ProxyServer means HTTP proxy for ALL protocols —
//!     browsers then speak plain HTTP to our SOCKS5 port and everything
//!     fails. A SOCKS-only endpoint MUST be written as "socks=host:port".
//!     (v2rayN gets away with the bare form because it also runs an HTTP
//!     inbound; we only have SOCKS5, so the prefix is mandatory.)
//!  2. A WM_SETTINGCHANGE broadcast does NOT refresh WinInet's cached proxy
//!     config — only InternetSetOption(SETTINGS_CHANGED)+REFRESH does.
//! ponytail: disabling wipes ProxyServer/Override instead of restoring
//! whatever was there before — same as v2rayN's fallback. Snapshot/restore
//! only if a user ever complains.

use std::process::Command;

const REG_PATH: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings";

fn reg(args: &[&str]) -> Result<String, String> {
    let out = Command::new("reg")
        .args(args)
        .output()
        .map_err(|e| format!("failed to run `reg`: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
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
            // The "socks=" prefix is MANDATORY for a SOCKS-only endpoint —
            // a bare host:port would be treated as an HTTP proxy (see docs).
            let proxy = format!("socks={server}");
            reg(&[
                "add", REG_PATH, "/v", "ProxyServer", "/t", "REG_SZ", "/d", proxy.as_str(),
                "/f",
            ])?;
            reg(&[
                "add", REG_PATH, "/v", "ProxyOverride", "/t", "REG_SZ", "/d", "<local>",
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
