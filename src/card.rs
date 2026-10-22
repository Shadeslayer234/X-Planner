//! X card math: day numbers, cell states and streaks.
//!
//! Rules (from DESIGN.md):
//! - Day N = days since the card's start date + 1.
//! - A past day with a mark is an X; a past day without one is a miss.
//! - Today, unmarked, is drawn dashed. Future days are empty.
//! - Streak = consecutive marked days ending yesterday, plus today if marked.

use crate::model::{Date, Project};

/// What one cell of the 7×7 grid shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellState {
    X,
    Miss,
    /// Today, not marked yet.
    Today,
    Future,
}

impl CellState {
    /// The integer the Slint UI uses for this state (see `ui/theme.slint`).
    pub fn as_int(self) -> i32 {
        match self {
            CellState::X => 0,
            CellState::Miss => 1,
            CellState::Today => 2,
            CellState::Future => 3,
        }
    }
}

impl Project {
    pub fn is_marked(&self, date: Date) -> bool {
        self.marks.binary_search(&date).is_ok()
    }

    /// Adds or removes the X for `date`, keeping `marks` sorted.
    pub fn toggle_mark(&mut self, date: Date) {
        match self.marks.binary_search(&date) {
            Ok(i) => {
                self.marks.remove(i);
            }
            Err(i) => self.marks.insert(i, date),
        }
    }

    /// Day number on the card for `today`. 0 or less means the card hasn't
    /// started; more than `length` means it's finished.
    pub fn day_number(&self, today: Date) -> i64 {
        (today - self.card.start_date).num_days() + 1
    }

    pub fn card_length(&self) -> i64 {
        self.card.length.max(1) as i64
    }

    /// True while today falls inside the card.
    pub fn card_active(&self, today: Date) -> bool {
        let n = self.day_number(today);
        n >= 1 && n <= self.card_length()
    }

    /// The state of every cell, day 1 first.
    pub fn cells(&self, today: Date) -> Vec<CellState> {
        let today_n = self.day_number(today);
        (1..=self.card_length())
            .map(|n| {
                let date = self.card.start_date + chrono::Duration::days(n - 1);
                if n < today_n {
                    if self.is_marked(date) {
                        CellState::X
                    } else {
                        CellState::Miss
                    }
                } else if n == today_n {
                    if self.is_marked(date) {
                        CellState::X
                    } else {
                        CellState::Today
                    }
                } else {
                    CellState::Future
                }
            })
            .collect()
    }

    /// Consecutive marked days ending yesterday, plus today if it's marked.
    /// Only days on the current card count.
    pub fn streak(&self, today: Date) -> u32 {
        let mut n = 0;
        let mut day = today - chrono::Duration::days(1);
        while day >= self.card.start_date && self.is_marked(day) {
            n += 1;
            day -= chrono::Duration::days(1);
        }
        if today >= self.card.start_date && self.is_marked(today) {
            n += 1;
        }
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Card, ColorName, Kind};

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    fn project(start: &str, marks: &[&str]) -> Project {
        Project {
            id: "p".into(),
            name: "P".into(),
            kind: Kind::Project,
            minimum: "1".into(),
            color: ColorName::Amber,
            card: Card { start_date: d(start), length: 49 },
            marks: marks.iter().map(|m| d(m)).collect(),
            archived: false,
            extra: Default::default(),
        }
    }

    #[test]
    fn day_number_starts_at_one() {
        let p = project("2026-10-01", &[]);
        assert_eq!(p.day_number(d("2026-10-01")), 1);
        assert_eq!(p.day_number(d("2026-10-22")), 22);
        assert_eq!(p.day_number(d("2026-09-30")), 0);
    }

    #[test]
    fn cells_follow_the_rules() {
        let p = project("2026-10-01", &["2026-10-01", "2026-10-03"]);
        let cells = p.cells(d("2026-10-04"));
        assert_eq!(cells.len(), 49);
        assert_eq!(&cells[..5], &[CellState::X, CellState::Miss, CellState::X, CellState::Today, CellState::Future]);
    }

    #[test]
    fn streak_ends_yesterday_plus_today() {
        let p = project("2026-10-01", &["2026-10-02", "2026-10-03"]);
        assert_eq!(p.streak(d("2026-10-04")), 2);
        let mut p2 = p.clone();
        p2.toggle_mark(d("2026-10-04"));
        assert_eq!(p2.streak(d("2026-10-04")), 3);
        // A miss yesterday breaks it, even with today marked.
        let p3 = project("2026-10-01", &["2026-10-01", "2026-10-04"]);
        assert_eq!(p3.streak(d("2026-10-04")), 1);
    }

    #[test]
    fn toggle_mark_keeps_marks_sorted() {
        let mut p = project("2026-10-01", &["2026-10-01", "2026-10-05"]);
        p.toggle_mark(d("2026-10-03"));
        assert_eq!(p.marks, vec![d("2026-10-01"), d("2026-10-03"), d("2026-10-05")]);
        p.toggle_mark(d("2026-10-03"));
        assert_eq!(p.marks, vec![d("2026-10-01"), d("2026-10-05")]);
    }
}
