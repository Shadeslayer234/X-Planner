//! The shutdown shortcut: a `.lnk` that runs `xplanner.exe --shutdown-check`
//! with the power icon.
//!
//! Windows doesn't let apps pin things to the taskbar, so we create the
//! shortcut in the Start menu and open its folder; the user right-clicks it
//! and picks "Pin to taskbar".
//!
//! The shortcut and the check dialog get their own AppUserModelID. Without
//! it, the taskbar would group the pinned button with the main X Planner
//! window, and clicking it would just switch to that window.

use std::path::PathBuf;

use windows::core::{Interface, HSTRING};
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Variant::VT_LPWSTR;
use windows::Win32::Storage::EnhancedStorage::PKEY_AppUserModel_ID;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, IPersistFile, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
use windows::Win32::UI::Shell::{IShellLinkW, SHStrDupW, SetCurrentProcessExplicitAppUserModelID, ShellLink};

pub const APP_ID: &str = "XPlanner.App";
pub const SHUTDOWN_APP_ID: &str = "XPlanner.ShutdownCheck";

/// Sets this process's AppUserModelID (taskbar grouping).
pub fn set_process_app_id(id: &str) {
    unsafe {
        let _ = SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(id));
    }
}

/// Where the shortcut goes: the user's Start menu.
pub fn shortcut_path() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    Some(PathBuf::from(appdata).join(r"Microsoft\Windows\Start Menu\Programs\X Planner Shutdown.lnk"))
}

/// Creates (or replaces) the shortcut and returns its path.
pub fn create_shutdown_shortcut() -> windows::core::Result<PathBuf> {
    let not_found = || windows::core::Error::from_hresult(windows::Win32::Foundation::E_FAIL);
    let exe = std::env::current_exe().map_err(|_| not_found())?;
    let path = shortcut_path().ok_or_else(not_found)?;
    unsafe {
        // Fine if COM is already initialised on this thread.
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        link.SetPath(&HSTRING::from(exe.as_os_str()))?;
        link.SetArguments(&HSTRING::from("--shutdown-check"))?;
        link.SetDescription(&HSTRING::from("Shut down after checking tomorrow's plan"))?;
        if let Some(dir) = exe.parent() {
            link.SetWorkingDirectory(&HSTRING::from(dir.as_os_str()))?;
        }
        // A negative index means "icon resource with this id"; 2 is the
        // power icon (see build.rs).
        link.SetIconLocation(&HSTRING::from(exe.as_os_str()), -2)?;

        let store: IPropertyStore = link.cast()?;
        // A string PROPVARIANT. The windows crate clears it on drop, which
        // frees the string with the COM allocator, so allocate it there.
        let mut value = PROPVARIANT::default();
        (*value.Anonymous.Anonymous).vt = VT_LPWSTR;
        (*value.Anonymous.Anonymous).Anonymous.pwszVal = SHStrDupW(&HSTRING::from(SHUTDOWN_APP_ID))?;
        store.SetValue(&PKEY_AppUserModel_ID, &value)?;
        store.Commit()?;

        let file: IPersistFile = link.cast()?;
        file.Save(&HSTRING::from(path.as_os_str()), true)?;
    }
    Ok(path)
}
