# Keymap

| Key | Action |
|---|---|
| `1`–`9` | Jump to that screen. |
| `Tab` / `Shift+Tab` | Cycle screens. |
| `:` or `Ctrl+P` | Open the command palette. |
| `Ctrl+N` | Open the node picker. |
| `Ctrl+L` | Toggle the bottom log pane. |
| `Ctrl+A` | Toggle the alerts popup. |
| `?` | Open the help overlay. |
| `↑ ↓` or `j k` | Move selection in the active list. |
| `Enter` / click | Drill into the selected row (or switch node in S15 / picker). |
| `PgUp` / `PgDn` | Page selection ±10 rows. |
| `Home` / `End` | First / last row. |
| `r` | Re-poll fleet (S15) · run `rchash` bench (S4). |
| `c` | Check all pins (S10). |
| `s` | Cycle pin sort mode (S10). |
| `Esc` | Close any overlay or drill. |
| Click a tab | Same as `1`–`9` for that index. |

## Focus-aware shortcuts

Global key shortcuts (digit screen-jumps, Tab cycling, `?`, arrow
nav) suppress themselves when a text input owns keyboard focus.
Typing `5` into a text field doesn't jump to S5; pressing `↓` in
a feed-timeline owner field moves the caret, not the selection.

`Ctrl`-modified shortcuts (`Ctrl+P`, `Ctrl+L`, `Ctrl+A`, `Ctrl+N`)
work regardless of focus — those are unambiguous.

## Comparison with bee-tui

| Concept | bee-tui | beegui |
|---|---|---|
| Open palette | `:` | `:` or `Ctrl+P` |
| Switch screen | `Alt+1`–`Alt+9` or Tab | `1`–`9` or Tab |
| Switch node | `Ctrl+N` picker / S15 Enter | `Ctrl+N` picker / S15 Enter |
| Help | `?` | `?` |
| Quit | `:q` or `Ctrl+C` | `:q` or window close |
| Log pane | `Ctrl+L` (also `Shift+L` fullscreen) | `Ctrl+L` |
