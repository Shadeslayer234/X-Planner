//! Turning the PC off.
//!
//! Debug builds only print what they would do, so testing the shutdown
//! check never switches the machine off. Set `XPLANNER_REAL_SHUTDOWN=1` to
//! shut down for real from a debug build.

use windows::core::w;
use windows::Win32::Foundation::{CloseHandle, HANDLE, LUID};
use windows::Win32::Security::{
    AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES, SE_PRIVILEGE_ENABLED, TOKEN_ADJUST_PRIVILEGES,
    TOKEN_PRIVILEGES, TOKEN_QUERY,
};
use windows::Win32::System::Shutdown::{
    ExitWindowsEx, EWX_HYBRID_SHUTDOWN, EWX_POWEROFF, EWX_SHUTDOWN, SHTDN_REASON_FLAG_PLANNED,
    SHTDN_REASON_MAJOR_OTHER,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

fn dry_run() -> bool {
    cfg!(debug_assertions) && std::env::var_os("XPLANNER_REAL_SHUTDOWN").is_none()
}

/// Shuts Windows down the same way the Start menu does (with fast startup
/// when it's enabled). Returns false if Windows refused.
pub fn shut_down() -> bool {
    if dry_run() {
        eprintln!("[dry run] Windows would shut down now.");
        return true;
    }
    unsafe {
        // Apps need the shutdown privilege switched on in their token first.
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut luid = LUID::default();
        let _ = LookupPrivilegeValueW(None, w!("SeShutdownPrivilege"), &mut luid);
        let privileges = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES { Luid: luid, Attributes: SE_PRIVILEGE_ENABLED }],
        };
        let _ = AdjustTokenPrivileges(token, false, Some(&privileges), 0, None, None);
        let _ = CloseHandle(token);

        let reason = SHTDN_REASON_MAJOR_OTHER | SHTDN_REASON_FLAG_PLANNED;
        ExitWindowsEx(EWX_SHUTDOWN | EWX_POWEROFF | EWX_HYBRID_SHUTDOWN, reason).is_ok()
            || ExitWindowsEx(EWX_SHUTDOWN | EWX_POWEROFF, reason).is_ok()
    }
}
