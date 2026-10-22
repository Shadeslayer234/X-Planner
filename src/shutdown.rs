//! The shutdown check (`xplanner.exe --shutdown-check`).
//!
//! It runs as its own short-lived process, separate from the main app:
//! reads data.json, shows what's missing, and only calls a real shutdown
//! when you confirm (or after a 5-second countdown when all is set).

use std::rc::Rc;
use std::time::Duration;

use slint::ComponentHandle;

use crate::model::Data;
use crate::win;
use crate::ShutdownWindow;

/// "A", "A and B", "A, B and C".
fn join_names(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [one] => one.to_string(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// What the dialog shows.
#[derive(Debug, PartialEq)]
pub struct Check {
    pub all_set: bool,
    pub heading: String,
    pub message: String,
    pub show_x: bool,
    pub x_ok: bool,
    pub x_detail: String,
    pub show_plan: bool,
    pub plan_ok: bool,
    pub plan_detail: String,
    /// True when planning is what's missing ("Plan now"), false when it's
    /// only an X ("Open X Planner").
    pub needs_plan: bool,
}

pub fn check(data: &Data, today: crate::model::Date) -> Check {
    let tomorrow = today + chrono::Duration::days(1);
    let weekday = tomorrow.format("%A").to_string();

    // Projects planned for tomorrow. Habits never block shutdown.
    let (planned, total) = data.planned_count(tomorrow);
    let unplanned: Vec<&str> =
        data.plannable().filter(|p| !data.is_planned(tomorrow, &p.id)).map(|p| p.name.as_str()).collect();

    // Optionally: today's X for every project and habit with an active card.
    let check_x = data.settings.shutdown_check.check_today_x;
    let with_card: Vec<_> = data.active_projects().filter(|p| p.card_active(today)).collect();
    let unmarked: Vec<&str> =
        with_card.iter().filter(|p| !p.is_marked(today)).map(|p| p.name.as_str()).collect();
    let x_ok = !check_x || unmarked.is_empty();

    let mut message = Vec::new();
    if !unplanned.is_empty() {
        let verb = if unplanned.len() == 1 { "has" } else { "have" };
        message.push(format!("{} {verb} no tasks for {weekday}. Habits don't need planning.", join_names(&unplanned)));
    }
    if !x_ok {
        message.push(format!("Today's X isn't marked for {}.", join_names(&unmarked)));
    }

    let all_set = unplanned.is_empty() && x_ok;
    let best_streak = with_card.iter().map(|p| p.streak(today)).max().unwrap_or(0);
    Check {
        all_set,
        heading: if all_set { format!("All set for {weekday}") } else { "Before you shut down".into() },
        message: message.join(" "),
        show_x: check_x && !with_card.is_empty(),
        x_ok,
        x_detail: if unmarked.is_empty() {
            format!("{best_streak}-day streak")
        } else {
            format!("{} of {}", with_card.len() - unmarked.len(), with_card.len())
        },
        show_plan: total > 0,
        plan_ok: planned == total,
        plan_detail: format!("{planned} of {total}"),
        needs_plan: !unplanned.is_empty(),
    }
}

/// Opens the main app on `signal`: asks the running copy, or starts one.
fn open_main(plan: bool) {
    let signal = if plan { win::instance::Signal::Plan } else { win::instance::Signal::Show };
    if win::instance::is_running() && win::instance::signal(signal) {
        return;
    }
    if let Ok(exe) = std::env::current_exe() {
        let mut cmd = std::process::Command::new(exe);
        if plan {
            cmd.arg("--plan");
        }
        let _ = cmd.spawn();
    }
}

pub fn run(data: &Data) -> Result<(), slint::PlatformError> {
    win::shortcut::set_process_app_id(win::shortcut::SHUTDOWN_APP_ID);
    let c = check(data, crate::app::today());
    let ui = ShutdownWindow::new()?;
    ui.set_all_set(c.all_set);
    ui.set_heading(c.heading.into());
    ui.set_message(c.message.into());
    ui.set_show_x(c.show_x);
    ui.set_x_ok(c.x_ok);
    ui.set_x_detail(c.x_detail.into());
    ui.set_show_plan(c.show_plan);
    ui.set_plan_ok(c.plan_ok);
    ui.set_plan_detail(c.plan_detail.into());
    ui.set_fix_label(if c.needs_plan { "Plan now" } else { "Open X Planner" }.into());

    let shut_down = |ui: &ShutdownWindow| {
        let _ = ui.hide();
        if !win::power::shut_down() {
            eprintln!("Windows refused to shut down.");
        }
        let _ = slint::quit_event_loop();
    };

    let weak = ui.as_weak();
    ui.on_shut_down(move || {
        if let Some(ui) = weak.upgrade() {
            shut_down(&ui);
        }
    });
    ui.on_cancel(|| {
        let _ = slint::quit_event_loop();
    });
    let needs_plan = c.needs_plan;
    ui.on_fix(move || {
        open_main(needs_plan);
        let _ = slint::quit_event_loop();
    });

    // All set: count down from 5, then shut down.
    let timer = Rc::new(slint::Timer::default());
    if c.all_set {
        let weak = ui.as_weak();
        timer.start(slint::TimerMode::Repeated, Duration::from_secs(1), move || {
            let Some(ui) = weak.upgrade() else { return };
            let left = ui.get_countdown() - 1;
            ui.set_countdown(left.max(0));
            if left <= 0 {
                shut_down(&ui);
            }
        });
    }

    ui.show()?;
    win::window::set_app_icon(ui.window());
    let weak = ui.as_weak();
    win::window::repaint_after_restore(ui.window(), move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_repaint(!ui.get_repaint());
        }
    });
    center(&ui);
    slint::run_event_loop_until_quit()?;
    drop(timer);
    Ok(())
}

/// Centres the dialog on the primary screen and gives it focus.
fn center(ui: &ShutdownWindow) {
    let weak = ui.as_weak();
    // Wait one turn of the event loop so the window has its real size.
    slint::Timer::single_shot(Duration::ZERO, move || {
        let Some(ui) = weak.upgrade() else { return };
        use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
        let size = ui.window().size();
        let (sw, sh) = unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) };
        let x = (sw - size.width as i32) / 2;
        let y = (sh - size.height as i32) / 2;
        ui.window().set_position(slint::PhysicalPosition::new(x, y));
        win::window::with_hwnd(&ui, win::window::bring_to_front);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ColorName, Kind};

    #[test]
    fn habits_never_block_and_x_is_optional() {
        let today: crate::model::Date = "2026-10-22".parse().unwrap();
        let tomorrow = today + chrono::Duration::days(1);
        let mut data = Data::default();
        let pg = data.create_project("Poltergeist", Kind::Project, "15 min", ColorName::Amber, today);
        let md = data.create_project("Meditation", Kind::Habit, "10 min", ColorName::Rose, today);
        data.settings.shutdown_check.check_today_x = false;

        let c = check(&data, today);
        assert!(!c.all_set && c.needs_plan);
        assert_eq!(c.message, "Poltergeist has no tasks for Friday. Habits don't need planning.");

        data.add_task(tomorrow, &pg, "Fix the door");
        assert!(check(&data, today).all_set);

        // With the X check on, the unmarked habit matters.
        data.settings.shutdown_check.check_today_x = true;
        let c = check(&data, today);
        assert!(!c.all_set && !c.needs_plan);
        assert_eq!(c.x_detail, "0 of 2");
        for id in [&pg, &md] {
            data.project_mut(id).unwrap().toggle_mark(today);
        }
        assert!(check(&data, today).all_set);
        assert_eq!(join_names(&["A", "B", "C"]), "A, B and C");
    }
}
