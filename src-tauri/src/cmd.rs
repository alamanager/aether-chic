//! Spawning console-subsystem helpers (reg, tasklist, taskkill, …) from a
//! GUI app flashes a visible console window per spawn on Windows. This sets
//! CREATE_NO_WINDOW so they run invisibly. No-op on other platforms.
use std::process::Command;

pub fn silent(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000)
    }
    #[cfg(not(windows))]
    {
        cmd
    }
}
