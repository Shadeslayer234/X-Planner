//! The data model, matching the `data.json` schema (v1) in `design/DESIGN.md`.
//!
//! These structs are serialized with serde. Field names use camelCase in JSON
//! (`#[serde(rename_all = "camelCase")]`), so `start_date` becomes `startDate`.
//!
//! The file is a contract: Claude edits it during the evening check-in. Unknown
//! fields are kept (`extra`) so nothing another tool writes gets thrown away.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Dates are stored as "YYYY-MM-DD" strings. chrono's `NaiveDate` uses that
/// format with serde, so we can use it directly.
pub type Date = NaiveDate;

/// The whole `data.json` file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Data {
    pub version: u32,
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub projects: Vec<Project>,
    /// Keyed by date. `BTreeMap` keeps the dates sorted in the file.
    #[serde(default)]
    pub days: BTreeMap<Date, Day>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Default for Data {
    fn default() -> Self {
        Data {
            version: 1,
            settings: Settings::default(),
            projects: Vec::new(),
            days: BTreeMap::new(),
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub window_mode: WindowMode,
    pub open_at_login: bool,
    pub evening_reminder: EveningReminder,
    pub shutdown_check: ShutdownCheck,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            window_mode: WindowMode::Normal,
            open_at_login: true,
            evening_reminder: EveningReminder::default(),
            shutdown_check: ShutdownCheck::default(),
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum WindowMode {
    #[default]
    Normal,
    OnTop,
    Pinned,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EveningReminder {
    pub enabled: bool,
    /// 24-hour "HH:MM".
    pub time: String,
}

impl Default for EveningReminder {
    fn default() -> Self {
        EveningReminder { enabled: true, time: "20:30".into() }
    }
}

impl EveningReminder {
    /// The reminder time, or 20:30 if the string in the file is malformed.
    pub fn parsed_time(&self) -> chrono::NaiveTime {
        chrono::NaiveTime::parse_from_str(&self.time, "%H:%M")
            .unwrap_or_else(|_| chrono::NaiveTime::from_hms_opt(20, 30, 0).unwrap())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShutdownCheck {
    pub check_today_x: bool,
}

impl Default for ShutdownCheck {
    fn default() -> Self {
        ShutdownCheck { check_today_x: true }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    /// X card plus daily tasks plus evening planning.
    #[default]
    Project,
    /// X card only. Never needs planning.
    Habit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ColorName {
    #[default]
    Amber,
    Green,
    Blue,
    Rose,
}

impl ColorName {
    pub const ALL: [ColorName; 4] =
        [ColorName::Amber, ColorName::Green, ColorName::Blue, ColorName::Rose];

    pub fn from_index(i: i32) -> ColorName {
        *ColorName::ALL.get(i.max(0) as usize).unwrap_or(&ColorName::Amber)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub kind: Kind,
    #[serde(default)]
    pub minimum: String,
    #[serde(default)]
    pub color: ColorName,
    pub card: Card,
    /// Dates with an X. Kept sorted and unique.
    #[serde(default)]
    pub marks: Vec<Date>,
    #[serde(default)]
    pub archived: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    pub start_date: Date,
    #[serde(default = "default_card_length")]
    pub length: u32,
}

fn default_card_length() -> u32 {
    49
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Day {
    #[serde(default)]
    pub tasks: Vec<Task>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub project_id: String,
    pub text: String,
    #[serde(default)]
    pub done: bool,
    /// The date this task was carried over from, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub carried_from: Option<Date>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
