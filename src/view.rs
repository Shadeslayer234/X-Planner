//! Turns `Data` into what the UI shows: fills the `AppState` global.
//!
//! This is a plain "render" step, like a React render function: every change
//! recomputes everything from the data. The data is tiny, so that's cheap.

use std::rc::Rc;

use chrono::{Duration, NaiveTime};
use slint::{Model, ModelRc, SharedString, VecModel};

use crate::model::{Data, Date, Kind, Project};
use crate::palette::pal;
use crate::{AppState, CardView, ProjectRow, TaskRow};

/// Updates `model` in place to hold `rows`. Rows that didn't change keep
/// their UI elements, so a focused text input inside a list isn't rebuilt.
pub fn sync<T: Clone + PartialEq + 'static>(model: &ModelRc<T>, rows: Vec<T>) -> ModelRc<T> {
    if let Some(vec) = model.as_any().downcast_ref::<VecModel<T>>() {
        for (i, row) in rows.iter().enumerate() {
            if i < vec.row_count() {
                if vec.row_data(i).as_ref() != Some(row) {
                    vec.set_row_data(i, row.clone());
                }
            } else {
                vec.push(row.clone());
            }
        }
        while vec.row_count() > rows.len() {
            vec.remove(vec.row_count() - 1);
        }
        return model.clone();
    }
    ModelRc::from(Rc::new(VecModel::from(rows)))
}

/// "2 tasks left", "all tasks done" or "no tasks today".
fn tasks_left(data: &Data, today: Date, p: &Project) -> String {
    let tasks = data.tasks_for(today, &p.id);
    let open = tasks.iter().filter(|t| !t.done).count();
    match (tasks.len(), open) {
        (0, _) => "no tasks today".into(),
        (_, 0) => "all tasks done".into(),
        (_, 1) => "1 task left".into(),
        (_, n) => format!("{n} tasks left"),
    }
}

/// The switcher's second line: "Project · 2 tasks left" or "Habit · nothing to plan".
pub fn kind_label(data: &Data, today: Date, p: &Project) -> String {
    match p.kind {
        Kind::Habit => "Habit · nothing to plan".into(),
        Kind::Project => format!("Project · {}", tasks_left(data, today, p)),
    }
}

/// "8:30 PM"
pub fn time_label(t: NaiveTime) -> String {
    t.format("%-I:%M %p").to_string()
}

/// "22 Oct 2026"
pub fn date_label(d: Date) -> String {
    d.format("%-d %b %Y").to_string()
}

pub fn card_view(data: &Data, today: Date, p: Option<&Project>) -> CardView {
    let Some(p) = p else {
        return CardView { pal: pal(Default::default()), ..Default::default() };
    };
    let n = p.day_number(today);
    let len = p.card_length();
    let day_label = if n < 1 {
        format!("Starts {}", if n == 0 { "tomorrow".into() } else { date_label(p.card.start_date) })
    } else if n > len {
        format!("Day {len} / {len}")
    } else {
        format!("Day {n} / {len}")
    };
    CardView {
        has_project: true,
        id: p.id.as_str().into(),
        name: p.name.as_str().into(),
        kind_label: kind_label(data, today, p).into(),
        is_habit: p.kind == Kind::Habit,
        minimum: p.minimum.as_str().into(),
        pal: pal(p.color),
        day_label: day_label.into(),
        streak: p.streak(today) as i32,
        marked: p.is_marked(today),
        can_mark: p.card_active(today),
        complete: n > len,
    }
}

/// Fills the shared UI state for the selected project.
pub fn fill(ui: &AppState, data: &Data, today: Date, selected: &str) {
    let tomorrow = today + Duration::days(1);
    let projects: Vec<&Project> = data.active_projects().collect();
    let current = projects.iter().copied().find(|p| p.id == selected);

    ui.set_weekday(today.format("%A").to_string().into());
    ui.set_date_label(date_label(today).into());

    let rows: Vec<ProjectRow> = projects
        .iter()
        .map(|p| ProjectRow {
            id: p.id.as_str().into(),
            name: p.name.as_str().into(),
            kind_label: kind_label(data, today, p).into(),
            pal: pal(p.color),
            marked: p.is_marked(today),
            selected: Some(p.id.as_str()) == current.map(|c| c.id.as_str()),
        })
        .collect();
    ui.set_projects(sync(&ui.get_projects(), rows));

    let marked = projects.iter().filter(|p| p.is_marked(today)).count();
    ui.set_x_today_label(format!("{marked}/{} X", projects.len()).into());

    ui.set_card(card_view(data, today, current));
    let cells: Vec<i32> = current.map(|p| p.cells(today).iter().map(|c| c.as_int()).collect()).unwrap_or_default();
    ui.set_cells(sync(&ui.get_cells(), cells));

    let tasks: Vec<TaskRow> = current
        .map(|p| data.tasks_for(today, &p.id))
        .unwrap_or_default()
        .into_iter()
        .map(|t| TaskRow { id: t.id.as_str().into(), text: t.text.as_str().into(), done: t.done })
        .collect();
    let done = tasks.iter().filter(|t| t.done).count();
    ui.set_done_label(format!("{done} of {} done", tasks.len()).into());
    ui.set_tasks(sync(&ui.get_tasks(), tasks));

    // Footer: is tomorrow planned?
    let (planned, total) = data.planned_count(tomorrow);
    ui.set_has_plannable(total > 0);
    ui.set_tomorrow_planned(total > 0 && planned == total);
    let reminder = &data.settings.evening_reminder;
    let reminder_part = if reminder.enabled {
        format!(" · reminder at {}", time_label(reminder.parsed_time()))
    } else {
        String::new()
    };
    if planned == total {
        ui.set_plan_title("Tomorrow is planned".into());
        let noun = if total == 1 { "project" } else { "projects" };
        ui.set_plan_detail(SharedString::from(format!("{planned} of {total} {noun} planned")));
    } else {
        ui.set_plan_title("Tomorrow isn't planned yet".into());
        let left = total - planned;
        let noun = if left == 1 { "project" } else { "projects" };
        ui.set_plan_detail(format!("{left} {noun} to plan{reminder_part}").into());
    }
}

/// Fills the evening planning page for planning tomorrow.
pub fn fill_plan(ui: &crate::PlanState, data: &Data, today: Date) {
    use crate::{HabitStatus, LeftoverRow, PlanGroup, TomorrowRow};
    let tomorrow = today + Duration::days(1);
    let weekday = tomorrow.format("%A").to_string();
    ui.set_weekday_upper(weekday.to_uppercase().into());
    ui.set_weekday(weekday.into());
    ui.set_date_label(date_label(tomorrow).into());
    let (planned, total) = data.planned_count(tomorrow);
    ui.set_summary(format!("{planned} of {total} planned").into());

    let groups: Vec<PlanGroup> = data
        .plannable()
        .map(|p| {
            let today_tasks = data.tasks_for(today, &p.id);
            let done = today_tasks.iter().filter(|t| t.done).count();
            let leftover: Vec<LeftoverRow> = today_tasks
                .iter()
                .filter(|t| !t.done)
                .map(|t| LeftoverRow {
                    id: t.id.as_str().into(),
                    text: t.text.as_str().into(),
                    carried: data.carried_copy(today, &t.id).is_some(),
                })
                .collect();
            let tomorrow_rows: Vec<TomorrowRow> = data
                .tasks_for(tomorrow, &p.id)
                .iter()
                .enumerate()
                .map(|(i, t)| TomorrowRow {
                    id: t.id.as_str().into(),
                    num: format!("{:02}", i + 1).into(),
                    text: t.text.as_str().into(),
                })
                .collect();
            let n = tomorrow_rows.len();
            PlanGroup {
                id: p.id.as_str().into(),
                name: p.name.as_str().into(),
                pal: pal(p.color),
                planned: n > 0,
                status_text: format!("{n} {}", if n == 1 { "task" } else { "tasks" }).into(),
                leftover: ModelRc::new(VecModel::from(leftover)),
                done_line: format!("{done} done today").into(),
                tomorrow: ModelRc::new(VecModel::from(tomorrow_rows)),
            }
        })
        .collect();
    let first_open = groups.iter().find(|g| !g.planned).or(groups.first()).map(|g| g.id.clone());
    ui.set_first_open(first_open.unwrap_or_default());
    ui.set_groups(sync_groups(&ui.get_groups(), groups));

    let habits: Vec<HabitStatus> = data
        .active_projects()
        .filter(|p| p.kind == Kind::Habit)
        .map(|p| HabitStatus {
            name: p.name.as_str().into(),
            pal: pal(p.color),
            status: if p.is_marked(today) { "Today's X marked" } else { "Today's X not marked yet" }.into(),
        })
        .collect();
    ui.set_habits(sync(&ui.get_habits(), habits));
}

/// Like `sync`, for plan groups. Their nested models are new objects each
/// time and never compare equal, so compare the rows inside them instead.
/// Unchanged groups keep their UI (and a half-typed task in the input).
fn sync_groups(model: &ModelRc<crate::PlanGroup>, rows: Vec<crate::PlanGroup>) -> ModelRc<crate::PlanGroup> {
    let same = |a: &crate::PlanGroup, b: &crate::PlanGroup| {
        a.id == b.id
            && a.name == b.name
            && a.pal == b.pal
            && a.status_text == b.status_text
            && a.done_line == b.done_line
            && a.leftover.iter().eq(b.leftover.iter())
            && a.tomorrow.iter().eq(b.tomorrow.iter())
    };
    if let Some(vec) = model.as_any().downcast_ref::<VecModel<crate::PlanGroup>>() {
        for (i, row) in rows.iter().enumerate() {
            match vec.row_data(i) {
                Some(old) if same(&old, row) => {}
                Some(_) => vec.set_row_data(i, row.clone()),
                None => vec.push(row.clone()),
            }
        }
        while vec.row_count() > rows.len() {
            vec.remove(vec.row_count() - 1);
        }
        return model.clone();
    }
    ModelRc::from(Rc::new(VecModel::from(rows)))
}
