//! "Open when I sign in to Windows": a value under
//! HKCU\Software\Microsoft\Windows\CurrentVersion\Run pointing at this exe.

use windows::core::{w, PCWSTR};
use windows::Win32::System::Registry::{RegDeleteKeyValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ};

const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const VALUE: PCWSTR = w!("XPlanner");

/// Adds or removes the Run entry. Returns false if the registry refused.
pub fn set(enabled: bool) -> bool {
    unsafe {
        if !enabled {
            let r = RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, VALUE);
            // "Not found" counts as success: it's already off.
            return r.is_ok() || r.0 == 2;
        }
        let Ok(exe) = std::env::current_exe() else { return false };
        let command = format!("\"{}\"", exe.display());
        let wide: Vec<u16> = command.encode_utf16().chain(std::iter::once(0)).collect();
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            RUN_KEY,
            VALUE,
            REG_SZ.0,
            Some(wide.as_ptr() as *const _),
            (wide.len() * 2) as u32,
        )
        .is_ok()
    }
}
