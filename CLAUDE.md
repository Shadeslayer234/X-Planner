# X Planner — project brief

A lightweight Windows desktop app built around the X effect (r/theXeffect).
Each project or habit gets a 49-day card (7×7). You mark one X per day when
you do at least the minimum. Projects also get a daily task list that's
planned each evening, and a shutdown button won't turn the PC off until
tomorrow is planned. Planned to be released as open source.

**The full UI spec is in `design/DESIGN.md`. Read it before writing UI code.**
`design/screens/*.dc.html` are the approved mockups, kept as reference
markup for layout, copy and colors. They use a design-tool runtime, so they
are not meant to run as-is.

## Stack (performance is a core requirement)
- **Rust only.** UI with **Slint** (`.slint` markup + Rust logic).
  - License: Slint under GPLv3 for this open-source app (our own code can stay MIT/Apache-2.0).
- Windows APIs through the `windows` crate. Get the HWND from Slint's winit window accessor.
- System tray: `tray-icon`. File watching: `notify`. JSON: `serde` + `serde_json`. Dates: `chrono`.
- Targets: near-zero idle CPU, low memory (aim well under 50 MB), fast cold start.
- Fully event-driven. No polling loops. The only timer is the next evening reminder.
- Release profile: `opt-level = 3`, `lto = true`, `codegen-units = 1`, `strip = true`, `panic = "abort"`.

## Data
One human-readable JSON file at `%APPDATA%\XPlanner\data.json` (schema in DESIGN.md).
- Write atomically: write to a temp file, then rename over the original.
- Watch the file for external changes and reload. Claude edits it during the evening check-in.

## Features for v1
1. Projects and habits, each with a 49-day X card, a minimum, a color, and a start date.
   - **Project:** X card plus daily tasks plus evening planning.
   - **Habit:** X card only. Nothing to plan, and it never blocks shutdown.
2. Main window with a project switcher dropdown, the X card, and today's tasks (projects only).
3. New project or habit form.
4. Evening planning: an accordion with one section per project. Each section has carry-over of unfinished tasks, tomorrow's tasks, and a "Plan <project> with Claude" button. Habits are listed at the bottom.
5. Settings: window mode, open at login, evening reminder time, shutdown check options, data file location.
6. Window modes:
   - Normal.
   - Always on top.
   - Pinned to desktop: attach to the desktop WorkerW layer so it survives Win+D, and use the compact layout. Fall back to a normal window if pinning fails.
7. Shutdown check:
   - The same exe launched with `--shutdown-check` shows the check dialog, then calls a real shutdown only when the user confirms.
   - Ship a `.lnk` shortcut for this with a power icon.
8. Starts at Windows sign-in through the HKCU `Run` registry key. Runs as a single instance (named mutex).

## Context
- I'm on Windows. I know JavaScript (MERN) and some C# (Unity). I'm new to Rust, so keep modules small and well commented.
- Keep v1 small. This tool supports my main work (game development); it shouldn't replace it.
