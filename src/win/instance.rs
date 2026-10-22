//! Single instance. The first copy of the app holds a named mutex. A second
//! copy signals the first through a named event (show the window, or open
//! evening planning) and exits.
//!
//! The first copy waits on those events on a background thread, which sleeps
//! inside the kernel until one is set: no polling.

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{
    CreateEventW, CreateMutexW, OpenEventW, SetEvent, WaitForMultipleObjects, EVENT_MODIFY_STATE, INFINITE,
};

const MUTEX: PCWSTR = w!("Local\\XPlanner.Instance");
const SHOW_EVENT: PCWSTR = w!("Local\\XPlanner.Show");
const PLAN_EVENT: PCWSTR = w!("Local\\XPlanner.Plan");

/// What another copy of the app asked for.
#[derive(Debug, Clone, Copy)]
pub enum Signal {
    Show,
    Plan,
}

/// Held for the life of the first instance.
pub struct InstanceGuard(HANDLE);

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// Returns a guard if this is the first instance, or `None` if another copy
/// is already running.
pub fn acquire() -> Option<InstanceGuard> {
    unsafe {
        let handle = CreateMutexW(None, true, MUTEX).ok()?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(handle);
            return None;
        }
        Some(InstanceGuard(handle))
    }
}

/// True if the main app is running (used by the shutdown check).
pub fn is_running() -> bool {
    use windows::Win32::System::Threading::{OpenMutexW, SYNCHRONIZATION_SYNCHRONIZE};
    unsafe {
        match OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, false, MUTEX) {
            Ok(h) => {
                let _ = CloseHandle(h);
                true
            }
            Err(_) => false,
        }
    }
}

/// Asks the running instance to do something. Returns false if it isn't
/// listening.
pub fn signal(what: Signal) -> bool {
    let name = match what {
        Signal::Show => SHOW_EVENT,
        Signal::Plan => PLAN_EVENT,
    };
    unsafe {
        match OpenEventW(EVENT_MODIFY_STATE, false, name) {
            Ok(h) => {
                let ok = SetEvent(h).is_ok();
                let _ = CloseHandle(h);
                ok
            }
            Err(_) => false,
        }
    }
}

/// Starts a thread that calls `on_signal` whenever another copy signals us.
pub fn listen(on_signal: impl Fn(Signal) + Send + 'static) {
    let (show, plan) = unsafe {
        // Auto-reset events: each SetEvent wakes the wait exactly once.
        match (CreateEventW(None, false, false, SHOW_EVENT), CreateEventW(None, false, false, PLAN_EVENT)) {
            (Ok(s), Ok(p)) => (s.0 as usize, p.0 as usize),
            _ => return,
        }
    };
    std::thread::Builder::new()
        .name("instance-signals".into())
        .spawn(move || {
            let handles = [HANDLE(show as *mut _), HANDLE(plan as *mut _)];
            loop {
                let r = unsafe { WaitForMultipleObjects(&handles, false, INFINITE) };
                match r.0.wrapping_sub(WAIT_OBJECT_0.0) {
                    0 => on_signal(Signal::Show),
                    1 => on_signal(Signal::Plan),
                    _ => return,
                }
            }
        })
        .ok();
}
