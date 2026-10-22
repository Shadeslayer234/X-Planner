//! "Pinned to desktop": attaches the widget window to the desktop.
//!
//! The widget is a top-level window *owned* by the desktop window (`Progman`,
//! or the `WorkerW` that holds the icons when a wallpaper app has split the
//! desktop). Windows keeps an owned window just above its owner, so:
//! - normally the widget sits on the desktop, below every app window, and
//! - Win+D brings the desktop to the front and the widget comes with it.
//!
//! (Making it a *child* of the desktop instead breaks: once you leave the
//! Win+D view, its drawing no longer reaches the screen.)
//!
//! The owner is set when winit creates the window, through Slint's window
//! attributes hook in main.rs.

use slint::winit_030::winit;
use windows::core::{w, BOOL};
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::UI::HiDpi::GetDpiForSystem;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowExW, SystemParametersInfoW, SPI_GETWORKAREA, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
};

thread_local! {
    /// True while the app creates the widget window. Slint's attributes hook
    /// sees every window the same way, so this is how it spots the widget.
    static CREATING_WIDGET: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Runs `f` (which creates and shows the widget) with the flag set.
pub fn while_creating_widget<T>(f: impl FnOnce() -> T) -> T {
    CREATING_WIDGET.with(|c| c.set(true));
    let result = f();
    CREATING_WIDGET.with(|c| c.set(false));
    result
}

pub fn creating_widget() -> bool {
    CREATING_WIDGET.with(|c| c.get())
}

/// The top-level desktop window that holds the icons' view.
fn desktop_window() -> Option<HWND> {
    unsafe extern "system" fn find(hwnd: HWND, out: LPARAM) -> BOOL {
        let view = FindWindowExW(Some(hwnd), None, w!("SHELLDLL_DefView"), None);
        if view.is_ok_and(|v| !v.is_invalid()) {
            *(out.0 as *mut HWND) = hwnd;
            return false.into();
        }
        true.into()
    }
    let mut desktop = HWND::default();
    unsafe {
        let _ = EnumWindows(Some(find), LPARAM(&mut desktop as *mut HWND as isize));
    }
    (!desktop.is_invalid()).then_some(desktop)
}

/// Top right of the primary screen's work area, in physical pixels.
fn top_right(width: f64, margin: f64) -> winit::dpi::PhysicalPosition<i32> {
    unsafe {
        let scale = GetDpiForSystem() as f64 / 96.0;
        let mut work = RECT::default();
        let _ = SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut work as *mut RECT as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
        winit::dpi::PhysicalPosition::new(
            work.right - ((width + margin) * scale).round() as i32,
            work.top + (margin * scale).round() as i32,
        )
    }
}

/// Adjusts the widget's window attributes before winit creates it: owned by
/// the desktop, at the top right of the screen, not on the taskbar. If the
/// desktop can't be found it stays a normal window in the same spot.
pub fn widget_attributes(attrs: winit::window::WindowAttributes) -> winit::window::WindowAttributes {
    use winit::platform::windows::WindowAttributesExtWindows;
    let attrs = attrs.with_position(top_right(300.0, 28.0)).with_skip_taskbar(true);
    match desktop_window() {
        Some(desktop) => attrs.with_owner_window(desktop.0 as isize),
        None => {
            eprintln!("Couldn't find the desktop; showing the widget as a normal window.");
            attrs
        }
    }
}
