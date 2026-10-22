//! The system tray icon and its menu.
//!
//! tray-icon reports clicks on its own callback. We forward each one to the
//! Slint event loop with `invoke_from_event_loop`, where the app handles it.

use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

#[derive(Debug, Clone, Copy)]
pub enum TrayAction {
    Open,
    Plan,
    Settings,
    Quit,
}

/// Creates the tray icon. Keep the returned value alive; dropping it removes
/// the icon. `on_action` runs on the UI thread.
pub fn create(on_action: fn(TrayAction)) -> Option<TrayIcon> {
    let open = MenuItem::new("Open X Planner", true, None);
    let plan = MenuItem::new("Plan tomorrow", true, None);
    let settings = MenuItem::new("Settings", true, None);
    let quit = MenuItem::new("Quit", true, None);
    let menu = Menu::new();
    menu.append_items(&[&open, &plan, &settings, &PredefinedMenuItem::separator(), &quit]).ok()?;

    let ids = [
        (open.id().clone(), TrayAction::Open),
        (plan.id().clone(), TrayAction::Plan),
        (settings.id().clone(), TrayAction::Settings),
        (quit.id().clone(), TrayAction::Quit),
    ];
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if let Some((_, action)) = ids.iter().find(|(id, _)| *id == event.id) {
            let action = *action;
            let _ = slint::invoke_from_event_loop(move || on_action(action));
        }
    }));
    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
            let _ = slint::invoke_from_event_loop(move || on_action(TrayAction::Open));
        }
    }));

    // Icon resource 1 in the exe is the app icon (see build.rs).
    let icon = tray_icon::Icon::from_resource(1, None).ok()?;
    TrayIconBuilder::new()
        .with_tooltip("X Planner")
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .build()
        .ok()
}
