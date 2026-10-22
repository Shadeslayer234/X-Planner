//! Loading, saving and watching `data.json`.
//!
//! - The file lives at `%APPDATA%\XPlanner\data.json`. Set the `XPLANNER_DATA`
//!   environment variable to use another file (handy for testing).
//! - Saves are atomic: write `data.json.tmp`, then rename it over the original.
//! - A watcher reports external edits (Claude edits the file during the
//!   evening check-in). Our own saves are recognised and ignored.

use std::fs;
use std::path::{Path, PathBuf};

use crate::model::Data;

pub struct Store {
    path: PathBuf,
    /// The exact text we last read or wrote, so we can tell our own saves
    /// apart from external edits.
    last_text: String,
    /// True when the file on disk didn't parse. The next save keeps a copy of
    /// it first, so a typo in a hand edit never loses data.
    disk_broken: bool,
    _watcher: Option<notify::RecommendedWatcher>,
}

pub fn default_path() -> PathBuf {
    if let Some(p) = std::env::var_os("XPLANNER_DATA") {
        return PathBuf::from(p);
    }
    let appdata = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| ".".into());
    appdata.join("XPlanner").join("data.json")
}

/// The result of reading the file.
pub enum Loaded {
    /// The file parsed.
    Data(Data),
    /// The file didn't exist yet.
    Missing,
    /// The text is the same as what we last read or wrote.
    Unchanged,
    /// The file exists but isn't valid. Holds the error message.
    Broken(String),
}

impl Store {
    pub fn new(path: PathBuf) -> Store {
        Store { path, last_text: String::new(), disk_broken: false, _watcher: None }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn dir(&self) -> PathBuf {
        self.path.parent().map(Path::to_path_buf).unwrap_or_else(|| ".".into())
    }

    /// Reads and parses the file.
    pub fn load(&mut self) -> Loaded {
        let text = match fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Loaded::Missing,
            Err(e) => return Loaded::Broken(e.to_string()),
        };
        if text == self.last_text {
            return Loaded::Unchanged;
        }
        // Notepad and PowerShell may add a UTF-8 byte order mark; serde
        // doesn't accept one.
        match serde_json::from_str::<Data>(text.trim_start_matches('\u{feff}')) {
            Ok(data) => {
                self.last_text = text;
                self.disk_broken = false;
                Loaded::Data(data)
            }
            Err(e) => {
                // A half-written file from another editor shows up here too;
                // the watcher will fire again once it's complete.
                self.disk_broken = true;
                Loaded::Broken(e.to_string())
            }
        }
    }

    /// Writes the data atomically. Does nothing if the text hasn't changed.
    pub fn save(&mut self, data: &Data) -> std::io::Result<()> {
        let mut text = serde_json::to_string_pretty(data)?;
        text.push('\n');
        if text == self.last_text {
            return Ok(());
        }
        fs::create_dir_all(self.dir())?;
        if self.disk_broken && self.path.exists() {
            let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
            let backup = self.path.with_file_name(format!("data.broken-{stamp}.json"));
            fs::copy(&self.path, backup)?;
            self.disk_broken = false;
        }
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, &text)?;
        // On Windows, std's rename replaces the target in one step
        // (MoveFileExW with MOVEFILE_REPLACE_EXISTING).
        fs::rename(&tmp, &self.path)?;
        self.last_text = text;
        Ok(())
    }

    /// Starts watching the file. `on_change` runs on the watcher's thread
    /// whenever `data.json` may have changed; call `load()` from the UI thread
    /// to pick the change up.
    pub fn watch(&mut self, on_change: impl Fn() + Send + 'static) -> notify::Result<()> {
        use notify::{EventKind, RecursiveMode, Watcher};
        let file_name = self.path.file_name().map(|n| n.to_os_string());
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            let Ok(event) = res else { return };
            if matches!(event.kind, EventKind::Access(_)) {
                return;
            }
            if event.paths.iter().any(|p| p.file_name().map(|n| n.to_os_string()) == file_name) {
                on_change();
            }
        })?;
        // Watch the folder, not the file: an atomic save replaces the file,
        // which would end a watch on the file itself.
        fs::create_dir_all(self.dir()).ok();
        watcher.watch(&self.dir(), RecursiveMode::NonRecursive)?;
        self._watcher = Some(watcher);
        Ok(())
    }

    /// Writes a `CLAUDE.md` next to the data file that explains the format to
    /// Claude during the evening check-in. Leaves an existing one alone.
    pub fn write_claude_notes(&self) {
        let notes = self.dir().join("CLAUDE.md");
        if !notes.exists() {
            fs::create_dir_all(self.dir()).ok();
            fs::write(notes, CLAUDE_NOTES).ok();
        }
    }
}

const CLAUDE_NOTES: &str = include_str!("../docs/data-format.md");
