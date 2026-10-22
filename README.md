# X Planner

A small Windows desktop app built around the
[X effect](https://www.reddit.com/r/theXeffect/). Each project or habit gets a
49-day card; mark one X per day when you do at least the minimum. Projects also
get a daily task list that you plan each evening, and a shutdown button won't
turn the PC off until tomorrow is planned.

Rust + [Slint](https://slint.dev). Event-driven, no polling. About 30 MB of
memory, and it opens in a fraction of a second.

## Download

Grab `xplanner.exe` from the
[latest release](https://github.com/Shadeslayer234/X-Planner/releases/latest)
and run it. It's a single file with no installer. Put it somewhere permanent
first, because "open at login" remembers where it is. The exe isn't signed, so
Windows SmartScreen may warn about it: click **More info → Run anyway**.

## Build and run

Needs Rust (`rustup`, stable, MSVC toolchain) and the Visual Studio C++ build
tools with a Windows SDK.

```
cargo run                 # debug build, with a console for log output
cargo build --release     # target\release\xplanner.exe
cargo test
```

Command-line options:

| Option | What it does |
|---|---|
| (none) | Starts the app, or shows the running copy. |
| `--plan` | Opens evening planning. |
| `--shutdown-check` | Shows the shutdown check, then shuts down if you confirm. |

Debug builds never really shut down: they print `[dry run]` instead. Set
`XPLANNER_REAL_SHUTDOWN=1` to override that.

## Data

Everything lives in one JSON file, `%APPDATA%\XPlanner\data.json`. Set
`XPLANNER_DATA` to use a different file, for example for testing. The format is
documented in [docs/data-format.md](docs/data-format.md). A copy is written
next to the data file as `CLAUDE.md`, so Claude knows the rules during the
evening check-in. The app watches the file and reloads when something else
edits it.

## Shutdown button

Settings → **Create shortcut** puts `X Planner Shutdown.lnk` in your Start
menu and opens its folder. Right-click it → **Pin to taskbar**. Windows doesn't
let apps pin themselves.

## Code layout

| Path | What's there |
|---|---|
| `src/model.rs` | The `data.json` structs. |
| `src/store.rs` | Atomic load and save, plus file watching. |
| `src/card.rs` | X card math: day numbers, cells, streaks. |
| `src/plan.rs` | Tasks, carry-over, planned checks, creating projects. |
| `src/view.rs` | Turns the data into what the UI shows. |
| `src/app.rs` | The running app: windows, callbacks, window modes. |
| `src/reminder.rs` | The one timer: evening reminder and midnight rollover. |
| `src/shutdown.rs` | The `--shutdown-check` dialog. |
| `src/claude.rs` | "Plan with Claude". |
| `src/win/` | Win32 code: tray, single instance, desktop pinning, shortcut, registry, power. |
| `ui/` | Slint markup. `theme.slint` holds the design tokens. |
| `design/` | The spec and the approved mockups. |
| `examples/make_icons.rs` | Draws `assets/app.ico` and `assets/power.ico`. |

## License

Our code: MIT ([LICENSE-MIT](LICENSE-MIT)) or Apache-2.0
([LICENSE-APACHE](LICENSE-APACHE)), at your option. Slint is used under the GPLv3, so distributed
builds of the app are GPLv3. IBM Plex fonts: SIL Open Font License
(`assets/fonts/OFL.txt`).
