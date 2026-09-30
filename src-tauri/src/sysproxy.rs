//! Windows system-proxy switch for the tunnel's SOCKS5 endpoint.
//!
//! Writes the per-user WinInet proxy (`HKCU\...\Internet Settings`) via the
//! built-in `reg` CLI — no new crates — then broadcasts `WM_SETTINGCHANGE`
//! so browsers pick it up immediately. Non-Windows targets get a clean
//! error instead of a silent no-op.
//! ponytail: registry diff is not tracked; if a toggle ever fails halfway,
//! re-flipping the switch re-applies both values from scratch.

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
fn broadcast_change() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
    };
    // "Environment" is the conventional lParam for a proxy/settings change.
    let setting: Vec<u16> = "Environment\0".encode_utf16().collect();
    unsafe {
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
        let stdout = reg(&["query", REG_PATH, "/v", "ProxyEnable"])?;
        Ok(stdout.contains("0x1"))
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
        if server.trim().is_empty() {
            return Err("proxy server address is empty".into());
        }
        if enabled {
            reg(&[
                "add",
                REG_PATH,
                "/v",
                "ProxyServer",
                "/t",
                "REG_SZ",
                "/d",
                server,
                "/f",
            ])?;
        }
        reg(&[
            "add",
            REG_PATH,
            "/v",
            "ProxyEnable",
            "/t",
            "REG_DWORD",
            "/d",
            if enabled { "1" } else { "0" },
            "/f",
        ])?;
        broadcast_change();
        Ok(())
    }
}
