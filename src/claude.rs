//! "Plan <project> with Claude": starts an evening check-in with Claude.
//!
//! - If the Claude Code CLI (`claude`) is on PATH, it opens in a new console
//!   in the data folder with the check-in prompt, and can edit data.json
//!   directly.
//! - Otherwise the prompt is copied to the clipboard and the Claude app (or
//!   claude.ai) opens, so you can paste it there.

use std::path::{Path, PathBuf};

use crate::model::{Data, Date};

/// What happened, for a short notice in the UI.
pub enum Launched {
    Cli,
    Clipboard,
}

fn prompt(data: &Data, project_id: &str, today: Date, data_path: &Path) -> String {
    let tomorrow = today + chrono::Duration::days(1);
    let name = data.project(project_id).map(|p| p.name.as_str()).unwrap_or(project_id);
    format!(
        "Evening check-in for my project {name} (projectId {project_id}) in X Planner. \
         My planner data is in {path}, and CLAUDE.md in the same folder explains the format. \
         Read today's tasks for this project under days {today}, then ask me briefly how today went \
         and what matters most tomorrow. When we agree, write tomorrow's tasks for projectId {project_id} \
         under days {tomorrow} in data.json, following the format notes. Three tasks or fewer is ideal. \
         Don't change anything else in the file.",
        path = data_path.display(),
    )
}

/// Finds `claude.exe` or `claude.cmd` on PATH.
fn find_cli() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .flat_map(|dir| ["claude.exe", "claude.cmd"].map(|f| dir.join(f)))
        .find(|p| p.is_file())
}

pub fn start(data: &Data, project_id: &str, today: Date, data_path: &Path) -> Launched {
    let text = prompt(data, project_id, today, data_path);
    if let Some(cli) = find_cli() {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
        let dir = data_path.parent().unwrap_or(Path::new("."));
        let spawned = std::process::Command::new(cli)
            .arg(&text)
            .current_dir(dir)
            .creation_flags(CREATE_NEW_CONSOLE)
            .spawn();
        if spawned.is_ok() {
            return Launched::Cli;
        }
    }
    crate::win::shell::copy_text(&text);
    // The Claude desktop app registers claude://; fall back to the web app.
    if !crate::win::shell::open("claude://") {
        crate::win::shell::open("https://claude.ai/new");
    }
    Launched::Clipboard
}
