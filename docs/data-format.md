# X Planner data file (`data.json`, version 1)

This folder belongs to X Planner, a small X-effect habit and project tracker.
`data.json` is its only data file. The app watches it and reloads when it
changes, so edits show up right away.

## Evening check-in rules (for Claude)
- Only add or edit tasks under `days["<tomorrow>"].tasks`, where `<tomorrow>`
  is tomorrow's date as `YYYY-MM-DD`. Don't change other days, projects,
  marks or settings unless the user asks.
- Each task needs a unique `id` (for example `"t_c01"`), the `projectId` of an
  existing project with `"kind": "project"`, the `text`, and `"done": false`.
  Add `"carriedFrom": "<today>"` when a task is an unfinished one from today.
- Habits (`"kind": "habit"`) never get tasks.
- Keep the file valid JSON with the same layout. Write the whole file at once.
- Today's log is `days["<today>"].tasks`: `done` shows what got finished.
  `projects[].marks` lists the dates that earned an X.

## Schema
```json
{
  "version": 1,
  "settings": {
    "windowMode": "normal | onTop | pinned",
    "openAtLogin": true,
    "eveningReminder": { "enabled": true, "time": "20:30" },
    "shutdownCheck": { "checkTodayX": true }
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
A project counts as planned for a date when `days[date].tasks` has at least one
task with its `projectId`.
