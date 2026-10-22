# X Planner — UI and behavior spec

Approved mockups are in `screens/`. Match their layout, copy and colors.
The values below are the source of truth.

## Visual tokens
| Token | Value | Use |
|---|---|---|
| bg | `#141619` | window background |
| surface | `#1C1F23` | cards, inputs |
| surface-2 | `#22262B` / `#2A2E34` | hover, secondary buttons |
| border | `#2A2E34` | card and window borders |
| divider | `#1F2227` | list row separators |
| text | `#E9EAEC` | primary text |
| text-muted | `#A0A6AD` | labels, secondary text |
| text-done | `#80868E` | completed tasks (strikethrough) |
| miss-dot | `#5A6068` | missed day marker |

Project colors (each has a main color, a tint for X cells, a border, and an ink for text on the main color):
| Name | main | tint | border | ink |
|---|---|---|---|---|
| amber | `#F0B445` | `#2A2418` | `#4A3C1E` | `#1A1405` |
| green | `#7CC48A` | `#1A2A1E` | `#2F4A35` | `#0D1A10` |
| blue | `#7FB0E0` | `#18222D` | `#2C4058` | `#0C1520` |
| rose | `#E58FA0` | `#2C1C20` | `#553039` | `#1E0C10` |

- **Type:** IBM Plex Sans for UI, IBM Plex Mono for dates, counts and streaks. Both are OFL-licensed; bundle them.
- **Shape and sizing:** square window corners, card radius 10, button radius 8. Every click target is at least 44 px. Icons are 1.8 px stroke line icons.
- **Section labels:** 12 px, semibold, uppercase, letter-spacing 0.08em, muted color.

## X card rules
- 49 cells in a 7×7 grid. Day N = days since the card's start date + 1.
- A past day with a mark shows an X in the project color on the tint background.
- A past day with no mark is a miss: a small grey dot. Never shown in red.
- Today, unmarked: dashed border in the project color, with the day number.
- Future days: empty, dim.
- Streak = consecutive marked days ending yesterday, plus today if it's marked.

## Screens

### Main window (`Main.dc.html`, 400 px wide)
- **Title bar:** weekday and date on the left; pin, settings and hide-to-tray buttons on the right. The window is frameless with a custom drag region.
- **Project switcher:**
  - The button shows a color dot, the project name, a kind line ("Project · 2 tasks left" or "Habit · nothing to plan"), an "n/m X" count for today, and a chevron.
  - The dropdown lists each project with its color dot, name, kind line, and either a small X (marked today) or "not yet".
  - At the bottom: "New project or habit".
- **X card:**
  - "Day N / 49", the minimum, and the streak (big mono number in the project color).
  - The grid.
  - "Mark today's X", which toggles to "Today's X is marked · Undo".
- **Projects only:** today's tasks with checkboxes, an "n of m done" count, and an add-task input.
- **Habits:** the line "Nothing to plan here. Do your <minimum>, then mark today's X."
- **Footer:** "Tomorrow isn't planned yet · n projects to plan · reminder at 8:30 PM" with a "Plan now" button.

### New project or habit (`NewProject.dc.html`)
- **Name.**
- **Kind,** two radio cards:
  - "Habit — just the X"
  - "Project — X plus daily tasks"
  - Can be changed later.
- **Minimum,** free text such as "10 minutes" or "1 lesson".
- **Color,** four swatches.
- **Start:** Today or Tomorrow.
- **Live preview** of the card.
- **Create button** label follows the kind.

### Evening planning (`EveningPlan.dc.html`)
- **Header:** "Plan <weekday>", the date, and "n of m planned".
- **One accordion section per project.** Only one is open at a time.
  - **Collapsed:** "n tasks" with a check, or "Not planned".
  - **Expanded:**
    - "Left from today": unfinished tasks, each with a "Carry over" checkbox.
    - Tomorrow's task list with remove buttons, and an add input.
    - A "Plan <project> with Claude" button. It opens an evening check-in with Claude, which reads today's log and writes that project's tasks into data.json.
- **Habits** are listed at the bottom with today's X status. No action is needed.
- **Footer:** "Remind me in 30 min" and "Save plan".

### Settings (`Settings.dc.html`)
- **Window mode,** three radio cards: Normal, Always on top, Pinned to desktop.
- **Open when I sign in to Windows.**
- **Evening planning reminder** and the reminder time.
- **Shutdown button:** "Create shortcut".
  - Windows doesn't let apps pin to the taskbar themselves. The button creates the `.lnk` and opens its folder, with a one-line hint: right-click → Pin to taskbar.
  - The mockup's "Pin to taskbar" label should be changed to match.
- **Also check today's X.**
- **Data file path** with "Open folder".

### Shutdown check (`ShutdownNotPlanned.dc.html`, `ShutdownReady.dc.html`)
- **Checks:**
  - Projects planned for tomorrow (n of m).
  - Optionally, today's X for every project and habit.
- **Not all planned:** "Before you shut down" with the actions "Shut down anyway", "Cancel" and "Plan now". Plan now opens evening planning.
- **All set:** "All set for <weekday>" with a 5-second countdown, "Cancel" and "Shut down now".
- Habits never block shutdown.

### Pinned to desktop, compact (`Desktop.dc.html`)
- 300 px widget.
- Project dropdown and streak.
- Small 7×7 grid next to "Day N / 49", the minimum, and "Mark today".
- Up to 3 tasks for projects; the habit line for habits.
- A row of color dots, filled for marked today and hollow for not yet.
- An "Open full view" link.

## data.json schema (v1)
```json
{
  "version": 1,
  "settings": {
    "windowMode": "normal | onTop | pinned",
    "openAtLogin": true,
    "eveningReminder": { "enabled": true, "time": "20:30" },
    "shutdownCheck": { "checkTodayX": true },
    "windowSize": { "width": 420, "height": 760 }
  },
  "projects": [
    {
      "id": "pg",
      "name": "Poltergeist",
      "kind": "project | habit",
      "minimum": "15 minutes",
      "color": "amber | green | blue | rose",
      "card": { "startDate": "2026-09-30", "length": 49 },
      "marks": ["2026-09-30", "2026-10-01"],
      "archived": false
    }
  ],
  "days": {
    "2026-10-23": {
      "tasks": [
        { "id": "t_01", "projectId": "pg", "text": "Fix the door interaction bug", "done": false, "carriedFrom": "2026-10-22" }
      ]
    }
  }
}
```
- `windowSize` is optional: the main window's last size in logical pixels, written by the app.
- A project counts as planned for a date when `days[date].tasks` has at least one task with its `projectId`.
- Claude's evening check-in only adds or edits tasks under `days[<tomorrow>]`. Keep the format stable and documented, because it's the contract.
