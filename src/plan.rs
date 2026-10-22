//! Operations on tasks, days and projects: everything that changes `Data`
//! apart from X marks (those live in `card.rs`).

use chrono::Duration;

use crate::model::{Card, ColorName, Data, Date, Kind, Project, Task};

/// A short unique id like "t_19a3f0c2b41". Uses the clock plus a counter so
/// two ids made in the same instant still differ.
pub fn new_id(prefix: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}_{:x}", nanos.wrapping_add(n))
}

/// Turns a name into a short id: "Unity pathway" -> "unity-pathway".
fn slug(name: &str) -> String {
    let mut s = String::new();
    for c in name.chars().flat_map(|c| c.to_lowercase()) {
        if c.is_ascii_alphanumeric() {
            s.push(c);
        } else if !s.ends_with('-') && !s.is_empty() {
            s.push('-');
        }
        if s.len() >= 24 {
            break;
        }
    }
    let s = s.trim_end_matches('-').to_string();
    if s.is_empty() {
        "project".into()
    } else {
        s
    }
}

impl Data {
    /// Projects and habits that aren't archived, in file order.
    pub fn active_projects(&self) -> impl Iterator<Item = &Project> {
        self.projects.iter().filter(|p| !p.archived)
    }

    pub fn project(&self, id: &str) -> Option<&Project> {
        self.projects.iter().find(|p| p.id == id)
    }

    pub fn project_mut(&mut self, id: &str) -> Option<&mut Project> {
        self.projects.iter_mut().find(|p| p.id == id)
    }

    /// Tasks for one project on one date, in order.
    pub fn tasks_for(&self, date: Date, project_id: &str) -> Vec<&Task> {
        self.days
            .get(&date)
            .map(|d| d.tasks.iter().filter(|t| t.project_id == project_id).collect())
            .unwrap_or_default()
    }

    /// A project counts as planned for a date when that day has at least one
    /// of its tasks.
    pub fn is_planned(&self, date: Date, project_id: &str) -> bool {
        self.days
            .get(&date)
            .is_some_and(|d| d.tasks.iter().any(|t| t.project_id == project_id))
    }

    /// Active projects (not habits) that need planning for `date`.
    pub fn plannable(&self) -> impl Iterator<Item = &Project> {
        self.active_projects().filter(|p| p.kind == Kind::Project)
    }

    /// (planned, total) for the projects that need planning on `date`.
    pub fn planned_count(&self, date: Date) -> (usize, usize) {
        let total = self.plannable().count();
        let planned = self.plannable().filter(|p| self.is_planned(date, &p.id)).count();
        (planned, total)
    }

    pub fn add_task(&mut self, date: Date, project_id: &str, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        self.days.entry(date).or_default().tasks.push(Task {
            id: new_id("t"),
            project_id: project_id.into(),
            text: text.into(),
            done: false,
            carried_from: None,
            extra: Default::default(),
        });
    }

    fn task_mut(&mut self, date: Date, task_id: &str) -> Option<&mut Task> {
        self.days.get_mut(&date)?.tasks.iter_mut().find(|t| t.id == task_id)
    }

    pub fn toggle_task(&mut self, date: Date, task_id: &str) {
        if let Some(t) = self.task_mut(date, task_id) {
            t.done = !t.done;
        }
    }

    pub fn remove_task(&mut self, date: Date, task_id: &str) {
        if let Some(day) = self.days.get_mut(&date) {
            day.tasks.retain(|t| t.id != task_id);
            if day.tasks.is_empty() && day.extra.is_empty() {
                self.days.remove(&date);
            }
        }
    }

    /// The copy of `task_id` (from `from`) that was carried to the next day,
    /// matched by project, text and `carriedFrom`.
    pub fn carried_copy(&self, from: Date, task_id: &str) -> Option<&Task> {
        let src = self.days.get(&from)?.tasks.iter().find(|t| t.id == task_id)?;
        let to = from + Duration::days(1);
        self.days.get(&to)?.tasks.iter().find(|t| {
            t.carried_from == Some(from) && t.project_id == src.project_id && t.text == src.text
        })
    }

    /// Carries an unfinished task from `from` to the next day, or removes the
    /// carried copy if it's already there.
    pub fn toggle_carry(&mut self, from: Date, task_id: &str) {
        let to = from + Duration::days(1);
        if let Some(copy) = self.carried_copy(from, task_id) {
            let id = copy.id.clone();
            self.remove_task(to, &id);
            return;
        }
        let Some(src) = self.days.get(&from).and_then(|d| d.tasks.iter().find(|t| t.id == task_id))
        else {
            return;
        };
        let copy = Task {
            id: new_id("t"),
            project_id: src.project_id.clone(),
            text: src.text.clone(),
            done: false,
            carried_from: Some(from),
            extra: Default::default(),
        };
        self.days.entry(to).or_default().tasks.push(copy);
    }

    /// Adds a new project or habit and returns its id.
    pub fn create_project(
        &mut self,
        name: &str,
        kind: Kind,
        minimum: &str,
        color: ColorName,
        start: Date,
    ) -> String {
        let base = slug(name);
        let mut id = base.clone();
        let mut n = 2;
        while self.projects.iter().any(|p| p.id == id) {
            id = format!("{base}-{n}");
            n += 1;
        }
        self.projects.push(Project {
            id: id.clone(),
            name: name.trim().into(),
            kind,
            minimum: minimum.trim().into(),
            color,
            card: Card { start_date: start, length: 49 },
            marks: Vec::new(),
            archived: false,
            extra: Default::default(),
        });
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    #[test]
    fn planning_and_carry_over() {
        let mut data = Data::default();
        let today = d("2026-10-22");
        let tomorrow = d("2026-10-23");
        let pg = data.create_project("Poltergeist", Kind::Project, "15 minutes", ColorName::Amber, today);
        data.create_project("Meditation", Kind::Habit, "10 minutes", ColorName::Rose, today);
        assert_eq!(data.planned_count(tomorrow), (0, 1));

        data.add_task(today, &pg, "Fix the door bug");
        let tid = data.tasks_for(today, &pg)[0].id.clone();
        data.toggle_carry(today, &tid);
        assert_eq!(data.planned_count(tomorrow), (1, 1));
        assert_eq!(data.tasks_for(tomorrow, &pg)[0].carried_from, Some(today));

        // Toggling again removes the copy, and the empty day goes away.
        data.toggle_carry(today, &tid);
        assert_eq!(data.planned_count(tomorrow), (0, 1));
        assert!(!data.days.contains_key(&tomorrow));
    }

    #[test]
    fn project_ids_are_unique_slugs() {
        let mut data = Data::default();
        let t = d("2026-10-22");
        assert_eq!(data.create_project("Unity pathway", Kind::Project, "", ColorName::Blue, t), "unity-pathway");
        assert_eq!(data.create_project("Unity pathway", Kind::Project, "", ColorName::Blue, t), "unity-pathway-2");
        assert_eq!(data.create_project("!!!", Kind::Habit, "", ColorName::Blue, t), "project");
    }
}
