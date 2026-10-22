//! X Planner entry point.
//!
//! Command line:
//! - no arguments: start the app (or show the running copy).
//! - `--plan`: start the app, or ask the running copy, on evening planning.
//! - `--shutdown-check`: show the shutdown check, then shut down if confirmed.

// Release builds are GUI apps with no console window. Debug builds keep the
// console so `eprintln!` output is visible.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod card;
mod claude;
mod model;
mod palette;
mod plan;
mod reminder;
mod shutdown;
mod store;
mod view;
mod win;

slint::include_modules!();

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let want_plan = args.iter().any(|a| a == "--plan");

    // The shutdown check is a separate, short-lived process. It only reads
    // the data, so it doesn't need the single-instance lock.
    if args.iter().any(|a| a == "--shutdown-check") {
        let data = match store::Store::new(store::default_path()).load() {
            store::Loaded::Data(d) => d,
            _ => model::Data::default(),
        };
        if let Err(e) = shutdown::run(&data) {
            eprintln!("Couldn't show the shutdown check: {e}");
        }
        return;
    }

    // One copy at a time. A second launch asks the first to show itself.
    let Some(_instance) = win::instance::acquire() else {
        let signal = if want_plan { win::instance::Signal::Plan } else { win::instance::Signal::Show };
        win::instance::signal(signal);
        return;
    };

    win::shortcut::set_process_app_id(win::shortcut::APP_ID);
    // The desktop widget is created owned by the desktop window.
    let backend = slint::BackendSelector::new().backend_name("winit".into()).with_winit_window_attributes_hook(|attrs| {
        if win::desktop::creating_widget() {
            win::desktop::widget_attributes(attrs)
        } else {
            attrs
        }
    });
    if let Err(e) = backend.select() {
        eprintln!("Couldn't start the winit backend: {e}");
        return;
    }
    let mut store = store::Store::new(store::default_path());
    let data = match store.load() {
        store::Loaded::Data(d) => d,
        store::Loaded::Missing | store::Loaded::Unchanged => {
            let d = model::Data::default();
            store.save(&d).ok();
            d
        }
        store::Loaded::Broken(e) => {
            // Start empty but don't overwrite the file until something
            // changes; the save then keeps a backup of the broken file.
            eprintln!("data.json didn't parse: {e}");
            model::Data::default()
        }
    };
    store.write_claude_notes();

    // Keep the sign-in entry pointing at this exe (it may have moved).
    // Debug builds leave the registry alone unless you toggle the setting.
    if !cfg!(debug_assertions) {
        win::autostart::set(data.settings.open_at_login);
    }

    let app = match app::App::new(data, store) {
        Ok(app) => app,
        Err(e) => {
            eprintln!("Couldn't create the window: {e}");
            return;
        }
    };

    // External edits of data.json (for example by Claude) reload the UI.
    let watched = app.state.borrow_mut().store.watch(|| {
        let _ = slint::invoke_from_event_loop(|| app::with_app(|app| app.reload()));
    });
    if let Err(e) = watched {
        eprintln!("Couldn't watch data.json for changes: {e}");
    }

    win::instance::listen(|signal| {
        let _ = slint::invoke_from_event_loop(move || app::on_signal(signal));
    });
    app.set_tray(win::tray::create(app::on_tray));

    // In pinned mode the desktop widget is the main view; the full window
    // opens on demand.
    app.apply_window_mode();
    let pinned = app.state.borrow().data.settings.window_mode == model::WindowMode::Pinned;
    if want_plan || !pinned {
        app.show_main(if want_plan { Page::Evening } else { Page::Main });
    }

    // Keep running with every window hidden (the app lives in the tray).
    if let Err(e) = slint::run_event_loop_until_quit() {
        eprintln!("Event loop error: {e}");
    }
}
