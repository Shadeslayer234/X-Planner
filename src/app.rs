//! The running app: owns the data and the windows, and handles UI callbacks.
//!
//! Every change goes through `App::change`, which edits the data, saves it
//! and re-renders the UI. External edits to data.json go through
//! `App::reload`.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use slint::ComponentHandle;

use crate::model::{Data, Date};
use crate::store::{Loaded, Store};
use crate::win;
use crate::reminder::{Reminder, Wake};
use crate::model::{Kind, WindowMode};
use crate::{view, AppState, AppWindow, DesktopWidget, Page, PlanState, SettingsState};

pub fn today() -> Date {
    chrono::Local::now().date_naive()
}

pub fn tomorrow() -> Date {
    today() + chrono::Duration::days(1)
}

pub struct State {
    pub data: Data,
    pub store: Store,
    /// Id of the project shown in the main window.
    pub selected: String,
}

pub struct App {
    pub state: RefCell<State>,
    pub main: AppWindow,
    tray: RefCell<Option<tray_icon::TrayIcon>>,
    reminder: Reminder,
    /// The compact desktop widget, while in "Pinned to desktop" mode.
    widget: RefCell<Option<DesktopWidget>>,
}

thread_local! {
    /// The app, reachable from callbacks that come in through the event loop
    /// (tray clicks, file changes, signals from a second instance).
    static APP: RefCell<Weak<App>> = RefCell::new(Weak::new());
}

/// Runs `f` with the app, if it's running. Call from the UI thread.
pub fn with_app(f: impl FnOnce(&Rc<App>)) {
    if let Some(app) = APP.with(|a| a.borrow().upgrade()) {
        f(&app);
    }
}

impl App {
    pub fn new(data: Data, store: Store) -> Result<Rc<App>, slint::PlatformError> {
        let main = AppWindow::new()?;
        let app = Rc::new(App {
            state: RefCell::new(State { data, store, selected: String::new() }),
            main,
            tray: RefCell::new(None),
            reminder: Reminder::default(),
            widget: RefCell::new(None),
        });
        APP.with(|a| *a.borrow_mut() = Rc::downgrade(&app));
        app.bind_callbacks(&app.main.global::<AppState>());
        app.bind_plan_callbacks(&app.main.global::<PlanState>());
        app.bind_settings_callbacks(&app.main.global::<SettingsState>());
        let weak = app.main.as_weak();
        win::window::repaint_after_restore(app.main.window(), move || {
            if let Some(main) = weak.upgrade() {
                main.set_repaint(!main.get_repaint());
            }
        });
        app.refresh();
        app.arm_timer();
        Ok(app)
    }

    /// Re-renders every window from the data.
    pub fn refresh(&self) {
        let mut state = self.state.borrow_mut();
        let today = today();
        // Keep a valid selection: the current one, or the first project.
        if state.data.active_projects().all(|p| p.id != state.selected) {
            let first = state.data.active_projects().next().map(|p| p.id.clone());
            state.selected = first.unwrap_or_default();
        }
        let ui = self.main.global::<AppState>();
        view::fill(&ui, &state.data, today, &state.selected);
        view::fill_plan(&self.main.global::<PlanState>(), &state.data, today);
        let settings = &state.data.settings;
        let ui_settings = self.main.global::<SettingsState>();
        ui_settings.set_window_mode(match settings.window_mode {
            WindowMode::Normal => 0,
            WindowMode::OnTop => 1,
            WindowMode::Pinned => 2,
        });
        ui_settings.set_open_at_login(settings.open_at_login);
        ui_settings.set_reminder_enabled(settings.evening_reminder.enabled);
        ui_settings.set_reminder_time(settings.evening_reminder.time.as_str().into());
        ui_settings.set_check_today_x(settings.shutdown_check.check_today_x);
        ui_settings.set_data_path(state.store.path().display().to_string().into());
        ui.set_pinned_mode(state.data.settings.window_mode == crate::model::WindowMode::Pinned);
        if let Some(widget) = self.widget.borrow().as_ref() {
            view::fill(&widget.global::<AppState>(), &state.data, today, &state.selected);
        }
        self.main.set_on_top(state.data.settings.window_mode == crate::model::WindowMode::OnTop);
    }

    /// Edits the data, saves it, and re-renders.
    pub fn change(&self, f: impl FnOnce(&mut State)) {
        {
            let mut state = self.state.borrow_mut();
            f(&mut state);
            let State { data, store, .. } = &mut *state;
            if let Err(e) = store.save(data) {
                eprintln!("Couldn't save data.json: {e}");
            }
        }
        self.refresh();
    }

    /// Picks up an external edit of data.json.
    pub fn reload(&self) {
        let loaded = self.state.borrow_mut().store.load();
        match loaded {
            Loaded::Data(data) => {
                self.state.borrow_mut().data = data;
                self.refresh();
            }
            Loaded::Broken(e) => eprintln!("data.json didn't parse, keeping the last good copy: {e}"),
            Loaded::Missing | Loaded::Unchanged => {}
        }
    }

    pub fn set_tray(&self, tray: Option<tray_icon::TrayIcon>) {
        *self.tray.borrow_mut() = tray;
    }

    /// Shows the main window on `page` and brings it to the front.
    pub fn show_main(&self, page: Page) {
        self.refresh();
        self.open_page(page);
        if let Err(e) = self.main.show() {
            eprintln!("Couldn't show the window: {e}");
            return;
        }
        win::window::set_app_icon(self.main.window());
        self.fit_to_screen();
        win::window::with_hwnd(&self.main, win::window::bring_to_front);
    }

    /// Keeps the window shorter than the screen it's on.
    fn fit_to_screen(&self) {
        use slint::winit_030::WinitWindowAccessor;
        let max = self
            .main
            .window()
            .with_winit_window(|w| {
                let m = w.current_monitor()?;
                Some(m.size().height as f32 / m.scale_factor() as f32 - 80.0)
            })
            .flatten();
        if let Some(max) = max {
            self.main.set_win_height(max.clamp(560.0, 980.0));
        }
    }

    fn bind_callbacks(self: &Rc<Self>, ui: &AppState) {
        // Each callback holds a weak reference, so the window (owned by the
        // app) doesn't keep the app alive in a cycle.
        let weak = Rc::downgrade(self);
        let on = move |f: fn(&App)| {
            let weak = weak.clone();
            move || {
                if let Some(app) = weak.upgrade() {
                    f(&app);
                }
            }
        };
        let weak = Rc::downgrade(self);
        let on_str = move |f: fn(&App, &str)| {
            let weak = weak.clone();
            move |s: slint::SharedString| {
                if let Some(app) = weak.upgrade() {
                    f(&app, &s);
                }
            }
        };

        ui.on_select_project(on_str(|app, id| {
            app.state.borrow_mut().selected = id.to_string();
            app.refresh();
        }));
        ui.on_toggle_mark(on(|app| {
            app.change(|s| {
                let id = s.selected.clone();
                if let Some(p) = s.data.project_mut(&id) {
                    p.toggle_mark(today());
                }
            })
        }));
        ui.on_start_new_card(on(|app| {
            app.change(|s| {
                let id = s.selected.clone();
                if let Some(p) = s.data.project_mut(&id) {
                    p.card.start_date = today();
                }
            })
        }));
        ui.on_toggle_task(on_str(|app, id| app.change(|s| s.data.toggle_task(today(), id))));
        ui.on_add_task(on_str(|app, text| {
            app.change(|s| {
                let id = s.selected.clone();
                s.data.add_task(today(), &id, text);
            })
        }));
        ui.set_palettes(slint::ModelRc::new(slint::VecModel::from(
            crate::model::ColorName::ALL.iter().map(|c| crate::palette::pal(*c)).collect::<Vec<_>>(),
        )));
        ui.on_create_project({
            let weak = Rc::downgrade(self);
            move |name, habit, minimum, color, tomorrow| {
                let Some(app) = weak.upgrade() else { return };
                let kind = if habit { crate::model::Kind::Habit } else { crate::model::Kind::Project };
                let start = if tomorrow { today() + chrono::Duration::days(1) } else { today() };
                let color = crate::model::ColorName::from_index(color);
                app.change(|s| {
                    s.selected = s.data.create_project(&name, kind, &minimum, color, start);
                });
                app.main.global::<AppState>().set_page(Page::Main);
            }
        });
        ui.on_new_project(on(|app| app.show_main(Page::NewProject)));
        ui.on_plan_now(on(|app| app.open_page(Page::Evening)));
        ui.on_open_settings(on(|app| app.open_page(Page::Settings)));
        ui.on_hide_window(on(|app| {
            let _ = app.main.hide();
        }));
        // The pin button switches to the desktop widget and closes this
        // full view. Settings switches back.
        let weak = Rc::downgrade(self);
        ui.on_toggle_pin(move || {
            let Some(app) = weak.upgrade() else { return };
            if app.state.borrow().data.settings.window_mode != WindowMode::Pinned {
                app.set_window_mode(WindowMode::Pinned);
            }
            if app.widget.borrow().is_some() {
                let _ = app.main.hide();
            }
        });
        ui.on_start_drag(on(|app| win::window::start_drag(app.main.window())));
        ui.on_open_full_view(on(|app| app.show_main(Page::Main)));
    }
}

impl App {
    /// Switches the main window to `page`.
    pub fn open_page(&self, page: Page) {
        self.main.global::<PlanState>().set_notice("".into());
        self.main.global::<AppState>().set_page(page);
    }

    /// (Re)starts the timer for the next reminder or midnight.
    pub fn arm_timer(&self) {
        let settings = self.state.borrow().data.settings.evening_reminder.clone();
        self.reminder.arm(&settings, |wake| with_app(|app| app.on_wake(wake)));
    }

    fn on_wake(&self, wake: Wake) {
        match wake {
            // A new day: re-render so "today" moves on.
            Wake::Midnight => self.refresh(),
            Wake::Reminder => {
                self.reminder.fired();
                let tomorrow = today() + chrono::Duration::days(1);
                let (planned, total) = self.state.borrow().data.planned_count(tomorrow);
                if planned < total {
                    self.show_main(Page::Evening);
                }
            }
        }
        self.arm_timer();
    }

    fn bind_settings_callbacks(self: &Rc<Self>, ui: &SettingsState) {
        let weak = Rc::downgrade(self);
        let with = move || weak.clone();
        let w = with();
        ui.on_set_window_mode(move |mode| {
            let Some(app) = w.upgrade() else { return };
            let mode = match mode {
                1 => WindowMode::OnTop,
                2 => WindowMode::Pinned,
                _ => WindowMode::Normal,
            };
            app.set_window_mode(mode);
        });
        let w = with();
        ui.on_set_open_at_login(move |on| {
            let Some(app) = w.upgrade() else { return };
            if crate::win::autostart::set(on) {
                app.change(|s| s.data.settings.open_at_login = on);
            }
        });
        let w = with();
        ui.on_set_reminder_enabled(move |on| {
            let Some(app) = w.upgrade() else { return };
            app.change(|s| s.data.settings.evening_reminder.enabled = on);
            app.arm_timer();
        });
        let w = with();
        ui.on_set_reminder_time(move |text| {
            let Some(app) = w.upgrade() else { return text };
            let Ok(t) = chrono::NaiveTime::parse_from_str(text.trim(), "%H:%M") else {
                return app.state.borrow().data.settings.evening_reminder.time.as_str().into();
            };
            let time = t.format("%H:%M").to_string();
            app.change(|s| s.data.settings.evening_reminder.time = time.clone());
            app.arm_timer();
            time.into()
        });
        let w = with();
        ui.on_set_check_today_x(move |on| {
            if let Some(app) = w.upgrade() {
                app.change(|s| s.data.settings.shutdown_check.check_today_x = on);
            }
        });
        let w = with();
        ui.on_create_shortcut(move || {
            if let Some(app) = w.upgrade() {
                app.create_shortcut();
            }
        });
        let w = with();
        ui.on_open_data_folder(move || {
            if let Some(app) = w.upgrade() {
                crate::win::shell::reveal(app.state.borrow().store.path());
            }
        });
        let w = with();
        ui.on_rename_project(move |name| {
            let Some(app) = w.upgrade() else { return };
            if name.trim().is_empty() {
                return;
            }
            app.change(|s| {
                let id = s.selected.clone();
                if let Some(p) = s.data.project_mut(&id) {
                    p.name = name.trim().to_string();
                }
            });
        });
        let w = with();
        ui.on_set_minimum(move |minimum| {
            let Some(app) = w.upgrade() else { return };
            app.change(|s| {
                let id = s.selected.clone();
                if let Some(p) = s.data.project_mut(&id) {
                    p.minimum = minimum.trim().to_string();
                }
            });
        });
        let w = with();
        ui.on_set_habit(move |habit| {
            let Some(app) = w.upgrade() else { return };
            app.change(|s| {
                let id = s.selected.clone();
                if let Some(p) = s.data.project_mut(&id) {
                    p.kind = if habit { Kind::Habit } else { Kind::Project };
                }
            });
        });
        let w = with();
        ui.on_archive_project(move || {
            let Some(app) = w.upgrade() else { return };
            app.change(|s| {
                let id = s.selected.clone();
                if let Some(p) = s.data.project_mut(&id) {
                    p.archived = true;
                }
            });
            app.open_page(Page::Main);
        });
    }

    /// Saves and applies a window mode.
    pub fn set_window_mode(self: &Rc<Self>, mode: WindowMode) {
        self.change(|s| s.data.settings.window_mode = mode);
        self.apply_window_mode();
    }

    /// Shows or removes the desktop widget to match the window mode.
    /// (Always-on-top for the main window is applied in refresh.)
    pub fn apply_window_mode(self: &Rc<Self>) {
        let pinned = self.state.borrow().data.settings.window_mode == WindowMode::Pinned;
        if !pinned {
            if let Some(widget) = self.widget.borrow_mut().take() {
                let _ = widget.hide();
            }
            return;
        }
        if self.widget.borrow().is_some() {
            return;
        }
        // Winit creates the native window inside new() or show(); the flag
        // makes the attributes hook turn it into a desktop child.
        let created = win::desktop::while_creating_widget(|| {
            let widget = DesktopWidget::new()?;
            widget.show()?;
            Ok::<_, slint::PlatformError>(widget)
        });
        let widget = match created {
            Ok(w) => w,
            Err(e) => {
                eprintln!("Couldn't create the desktop widget: {e}");
                return;
            }
        };
        self.bind_callbacks(&widget.global::<AppState>());
        *self.widget.borrow_mut() = Some(widget);
        self.refresh();
        let widget = self.widget.borrow();
        let Some(widget) = widget.as_ref() else { return };
        // The window is created owned by the desktop; see
        // win::desktop::widget_attributes, called from main.rs.
        win::window::with_hwnd(widget, win::window::square_corners);
        // If Explorer restarts, the desktop (and the widget it owns) is
        // destroyed. Build a new widget once Explorer is back.
        use slint::winit_030::WinitWindowAccessor;
        widget.window().on_winit_window_event(|_, event| {
            if matches!(event, slint::winit_030::winit::event::WindowEvent::Destroyed) {
                slint::Timer::single_shot(std::time::Duration::from_secs(3), || {
                    with_app(|app| {
                        app.widget.borrow_mut().take();
                        app.apply_window_mode();
                    })
                });
            }
            slint::winit_030::EventResult::Propagate
        });
    }

    /// Creates the shutdown-check shortcut and shows it in Explorer.
    pub fn create_shortcut(&self) {
        let notice = match crate::win::shortcut::create_shutdown_shortcut() {
            Ok(path) => {
                crate::win::shell::reveal(&path);
                "Created in your Start menu. Right-click it → Pin to taskbar.".to_string()
            }
            Err(e) => format!("Couldn't create the shortcut: {}", e.message()),
        };
        self.main.global::<SettingsState>().set_shortcut_notice(notice.into());
    }

    fn bind_plan_callbacks(self: &Rc<Self>, ui: &PlanState) {
        let weak = Rc::downgrade(self);
        let with = move |f: fn(&App, &str)| {
            let weak = weak.clone();
            move |s: slint::SharedString| {
                if let Some(app) = weak.upgrade() {
                    f(&app, &s);
                }
            }
        };
        ui.on_toggle_carry(with(|app, id| app.change(|s| s.data.toggle_carry(today(), id))));
        ui.on_remove_task(with(|app, id| app.change(|s| s.data.remove_task(tomorrow(), id))));
        let weak = Rc::downgrade(self);
        ui.on_add_task(move |project, text| {
            if let Some(app) = weak.upgrade() {
                app.change(|s| s.data.add_task(tomorrow(), &project, &text));
            }
        });
        ui.on_plan_with_claude(with(|app, id| {
            let launched = {
                let s = app.state.borrow();
                crate::claude::start(&s.data, id, today(), s.store.path())
            };
            let notice = match launched {
                crate::claude::Launched::Cli => "Claude is open in a new window. Tasks it writes show up here.",
                crate::claude::Launched::Clipboard => {
                    "The check-in prompt is on your clipboard. Paste it into Claude. Tasks it writes to data.json show up here."
                }
            };
            app.main.global::<PlanState>().set_notice(notice.into());
        }));
        let weak = Rc::downgrade(self);
        ui.on_remind_later(move || {
            if let Some(app) = weak.upgrade() {
                app.reminder.snooze(30);
                app.arm_timer();
                app.open_page(Page::Main);
                let _ = app.main.hide();
            }
        });
        let weak = Rc::downgrade(self);
        ui.on_save_plan(move || {
            if let Some(app) = weak.upgrade() {
                app.open_page(Page::Main);
            }
        });
    }
}

/// Handles tray menu clicks.
pub fn on_tray(action: win::tray::TrayAction) {
    use win::tray::TrayAction;
    with_app(|app| match action {
        TrayAction::Open => app.show_main(Page::Main),
        TrayAction::Plan => app.show_main(Page::Evening),
        TrayAction::Settings => app.show_main(Page::Settings),
        TrayAction::Quit => {
            let _ = slint::quit_event_loop();
        }
    });
}

/// Handles a signal from a second copy of the app.
pub fn on_signal(signal: win::instance::Signal) {
    with_app(|app| match signal {
        win::instance::Signal::Show => app.show_main(Page::Main),
        win::instance::Signal::Plan => app.show_main(Page::Evening),
    });
}
