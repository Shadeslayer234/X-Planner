//! The app's one timer. It wakes the app at the next of:
//! - the evening reminder (or a "remind me in 30 min" snooze), and
//! - midnight, so "today" moves on while the app is open.
//!
//! After each wake-up it's armed again for the next one. Nothing polls.

use std::cell::Cell;

use chrono::{Duration, Local, NaiveDateTime};

use crate::model::{Date, EveningReminder};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Wake {
    Reminder,
    Midnight,
}

#[derive(Default)]
pub struct Reminder {
    timer: slint::Timer,
    /// Set by "Remind me in 30 min".
    snooze_until: Cell<Option<NaiveDateTime>>,
    /// The day the reminder last went off, so it fires once per evening.
    fired_on: Cell<Option<Date>>,
}

impl Reminder {
    pub fn snooze(&self, minutes: i64) {
        self.snooze_until.set(Some(Local::now().naive_local() + Duration::minutes(minutes)));
    }

    /// Works out the next wake-up.
    fn next(&self, settings: &EveningReminder, now: NaiveDateTime) -> (NaiveDateTime, Wake) {
        let today = now.date();
        let midnight = (today + Duration::days(1)).and_hms_opt(0, 0, 1).unwrap();
        let mut best = (midnight, Wake::Midnight);
        if settings.enabled {
            let reminder = match self.snooze_until.get() {
                Some(s) => Some(s),
                None if self.fired_on.get() != Some(today) => {
                    Some(today.and_time(settings.parsed_time())).filter(|t| *t > now)
                }
                None => None,
            };
            if let Some(t) = reminder.filter(|t| *t < best.0) {
                best = (t, Wake::Reminder);
            }
        }
        best
    }

    /// Starts the timer for the next wake-up. `on_wake` runs when it fires;
    /// it should handle the wake and call `arm` again.
    pub fn arm(&self, settings: &EveningReminder, on_wake: impl Fn(Wake) + 'static) {
        let now = Local::now().naive_local();
        let (at, wake) = self.next(settings, now);
        let delay = (at - now).to_std().unwrap_or_default();
        self.timer.start(slint::TimerMode::SingleShot, delay, move || on_wake(wake));
    }

    /// Records that the reminder went off.
    pub fn fired(&self) {
        self.snooze_until.set(None);
        self.fired_on.set(Some(Local::now().date_naive()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }

    #[test]
    fn picks_reminder_then_midnight() {
        let r = Reminder::default();
        let s = EveningReminder::default();
        assert_eq!(r.next(&s, at("2026-10-22 10:00")), (at("2026-10-22 20:30"), Wake::Reminder));
        // After the reminder time, only midnight is left.
        let (t, w) = r.next(&s, at("2026-10-22 21:00"));
        assert_eq!((t.date().to_string(), w), ("2026-10-23".into(), Wake::Midnight));
        // A snooze wins.
        r.snooze_until.set(Some(at("2026-10-22 21:30")));
        assert_eq!(r.next(&s, at("2026-10-22 21:00")), (at("2026-10-22 21:30"), Wake::Reminder));
    }
}
