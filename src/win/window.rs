//! Native window helpers: getting the HWND, dragging a frameless window,
//! square corners and the window icon.

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::winit_030::{winit, WinitWindowAccessor};
use windows::Win32::Foundation::HWND;

/// The Win32 handle of a Slint window, once winit has created it.
pub fn hwnd(window: &slint::Window) -> Option<HWND> {
    window
        .with_winit_window(|w| match w.window_handle().ok()?.as_raw() {
            RawWindowHandle::Win32(h) => Some(HWND(h.hwnd.get() as *mut _)),
            _ => None,
        })
        .flatten()
}

/// Runs `f` with the window's HWND. Winit creates native windows lazily
/// (after `show()`, once the event loop runs), so if there's no handle yet
/// this tries again from the event loop.
pub fn with_hwnd<C: slint::ComponentHandle + 'static>(component: &C, f: impl FnOnce(HWND) + 'static) {
    if let Some(h) = hwnd(component.window()) {
        f(h);
        return;
    }
    let weak = component.as_weak();
    slint::Timer::single_shot(std::time::Duration::ZERO, move || {
        if let Some(h) = weak.upgrade().and_then(|c| hwnd(c.window())) {
            f(h);
        }
    });
}

/// Starts moving the window with the mouse (call on mouse-down in a drag area).
pub fn start_drag(window: &slint::Window) {
    window.with_winit_window(|w| {
        let _ = w.drag_window();
    });
}

/// Sets the window and taskbar icon from icon resource 1 in the exe, and
/// keeps the window square (call right after `show()`).
pub fn set_app_icon(window: &slint::Window) {
    use winit::platform::windows::IconExtWindows;
    if let Some(h) = hwnd(window) {
        square_corners(h);
    }
    window.with_winit_window(|w| {
        if let Ok(icon) = winit::window::Icon::from_resource(1, None) {
            w.set_window_icon(Some(icon));
        }
    });
}

/// Gives a frameless window square corners. Windows 10 draws them square
/// already; Windows 11 rounds top-level windows unless asked not to.
pub fn square_corners(hwnd: HWND) {
    use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND};
    unsafe {
        let pref = DWMWCP_DONOTROUND;
        // Fails harmlessly on Windows 10, which doesn't know the attribute.
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &pref as *const _ as *const _,
            std::mem::size_of_val(&pref) as u32,
        );
    }
    hide_caption(hwnd);
}

/// Winit's frameless windows keep the WS_CAPTION style and hide the caption
/// by making the whole window client area. Windows can still paint a classic
/// title bar over the top when the window is activated, dragged, or its
/// title or icon changes. This subclass stops that.
pub fn hide_caption(hwnd: HWND) {
    use windows::Win32::UI::Shell::SetWindowSubclass;
    unsafe {
        // Installing the same subclass id twice just updates it.
        let _ = SetWindowSubclass(hwnd, Some(no_caption_proc), 1, 0);
    }
}

/// Window subclass that keeps Windows from painting the (hidden) caption.
unsafe extern "system" fn no_caption_proc(
    hwnd: HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
    _id: usize,
    _data: usize,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::Foundation::{LPARAM, LRESULT};
    use windows::Win32::UI::Shell::DefSubclassProc;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongW, SetWindowLongW, GWL_STYLE, WM_NCACTIVATE, WM_NCPAINT, WM_SETICON, WM_SETTEXT, WS_VISIBLE,
    };
    match msg {
        // lparam -1 tells DefWindowProc not to repaint the non-client area.
        WM_NCACTIVATE => DefSubclassProc(hwnd, msg, wparam, LPARAM(-1)),
        WM_NCPAINT => LRESULT(0),
        // Undocumented theme messages (WM_NCUAHDRAWCAPTION and
        // WM_NCUAHDRAWFRAME) that paint the caption and its buttons, for
        // example while the window is dragged.
        0x00AE | 0x00AF => LRESULT(0),
        // Setting the title or icon paints the caption directly. Hiding the
        // visible flag for the call skips that painting.
        WM_SETTEXT | WM_SETICON => {
            let style = GetWindowLongW(hwnd, GWL_STYLE);
            SetWindowLongW(hwnd, GWL_STYLE, style & !(WS_VISIBLE.0 as i32));
            let r = DefSubclassProc(hwnd, msg, wparam, lparam);
            SetWindowLongW(hwnd, GWL_STYLE, style);
            r
        }
        _ => DefSubclassProc(hwnd, msg, wparam, lparam),
    }
}

/// Redraws the whole window after it comes back from being minimized.
/// Windows throws away a minimized window's picture, but Slint only redraws
/// the parts it thinks changed, so the rest would stay blank (showing
/// whatever is behind the window). `repaint` must change something that
/// covers the whole window, which makes Slint draw all of it again.
pub fn repaint_after_restore(window: &slint::Window, repaint: impl Fn() + 'static) {
    let minimized = std::cell::Cell::new(false);
    window.on_winit_window_event(move |w, event| {
        // Minimizing and restoring both arrive as resizes. (A minimized
        // frameless window reports a 160 x 28 size, not zero.)
        if let winit::event::WindowEvent::Resized(_) = event {
            // Ask winit: Slint's own flag only updates after this handler.
            let now = w.with_winit_window(|w| w.is_minimized()).flatten().unwrap_or(false);
            if minimized.replace(now) && !now {
                repaint();
            }
        }
        slint::winit_030::EventResult::Propagate
    });
}

/// Brings a window to the front, even if another app has focus.
pub fn bring_to_front(hwnd: HWND) {
    use windows::Win32::UI::WindowsAndMessaging::{SetForegroundWindow, ShowWindow, SW_RESTORE};
    unsafe {
        let _ = ShowWindow(hwnd, SW_RESTORE);
        let _ = SetForegroundWindow(hwnd);
    }
}
